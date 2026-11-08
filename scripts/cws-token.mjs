#!/usr/bin/env node
// One-time: sign in with the Google account that owns the Chrome Web Store item and store the
// resulting refresh token as the CWS_REFRESH_TOKEN secret of this GitHub repository.
//
//   node scripts/cws-token.mjs path/to/client_secret_….json
//
// Uses the "Desktop app" OAuth client's loopback flow. The token is never printed.
import { execFileSync, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import http from "node:http";

const file = process.argv[2];
if (!file) {
  console.error("Usage: node scripts/cws-token.mjs <client_secret.json>");
  process.exit(1);
}
const { client_id, client_secret } = JSON.parse(readFileSync(file, "utf8")).installed;
const state = randomBytes(16).toString("hex");

const server = http.createServer();
server.listen(0, "127.0.0.1", () => {
  const redirect = `http://127.0.0.1:${server.address().port}`;
  const auth = new URL("https://accounts.google.com/o/oauth2/v2/auth");
  auth.search = new URLSearchParams({
    client_id,
    redirect_uri: redirect,
    response_type: "code",
    scope: "https://www.googleapis.com/auth/chromewebstore",
    access_type: "offline",
    prompt: "consent",
    state,
  });
  console.log("Opening your browser to sign in with Google…\nIf it doesn't open, visit:\n" + auth);
  spawn(process.platform === "win32" ? "explorer" : "open", [auth.toString()], { stdio: "ignore", detached: true }).unref();

  server.on("request", async (req, res) => {
    const q = new URL(req.url, redirect).searchParams;
    if (q.get("state") !== state || !q.get("code")) {
      res.end(q.get("error") ? `Sign-in failed: ${q.get("error")}` : "Waiting for Google…");
      if (q.get("error")) process.exit(1);
      return;
    }
    const r = await fetch("https://oauth2.googleapis.com/token", {
      method: "POST",
      body: new URLSearchParams({ code: q.get("code"), client_id, client_secret, redirect_uri: redirect, grant_type: "authorization_code" }),
    });
    const tokens = await r.json();
    if (!tokens.refresh_token) {
      res.end("Google didn't return a refresh token. Run the script again.");
      console.error("✗ No refresh token:", tokens.error_description || tokens.error || "unknown error");
      process.exit(1);
    }
    execFileSync("gh", ["secret", "set", "CWS_REFRESH_TOKEN", "-R", "bloopdex/blurt"], { input: tokens.refresh_token });
    res.end("Done. You can close this tab: CWS_REFRESH_TOKEN is saved in GitHub.");
    console.log("✓ Saved CWS_REFRESH_TOKEN to github.com/bloopdex/blurt secrets.");
    server.close();
  });
});
