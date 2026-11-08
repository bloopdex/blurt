#!/usr/bin/env node
// Upload a zip to the Chrome Web Store and submit it for review (Chrome Web Store API v2).
//
//   node scripts/cws-publish.mjs extension.zip
//
// Needs CWS_CLIENT_ID, CWS_CLIENT_SECRET, CWS_REFRESH_TOKEN (scripts/cws-token.mjs makes it),
// CWS_PUBLISHER_ID and CWS_EXTENSION_ID. The very first upload of a new item must be done by
// hand in the developer dashboard; the API can only update existing items.
import { readFileSync } from "node:fs";

const env = (k) => process.env[k] || fail(`Missing ${k}.`);
function fail(msg) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}

const zip = process.argv[2] || fail("Usage: node scripts/cws-publish.mjs <extension.zip>");
const API = "https://chromewebstore.googleapis.com";
const item = `publishers/${env("CWS_PUBLISHER_ID")}/items/${env("CWS_EXTENSION_ID")}`;

async function call(url, init, what) {
  const r = await fetch(url, init);
  const text = await r.text();
  if (!r.ok) fail(`${what} failed (${r.status}): ${text}`);
  return text ? JSON.parse(text) : {};
}

const { access_token } = await call(
  "https://oauth2.googleapis.com/token",
  {
    method: "POST",
    body: new URLSearchParams({
      client_id: env("CWS_CLIENT_ID"),
      client_secret: env("CWS_CLIENT_SECRET"),
      refresh_token: env("CWS_REFRESH_TOKEN"),
      grant_type: "refresh_token",
    }),
  },
  "Getting an access token",
);
const auth = { Authorization: `Bearer ${access_token}` };

let upload = await call(`${API}/upload/v2/${item}:upload`, { method: "POST", headers: auth, body: readFileSync(zip) }, "Upload");
// Large uploads are processed asynchronously.
for (let i = 0; upload.uploadState === "IN_PROGRESS" && i < 30; i++) {
  await new Promise((r) => setTimeout(r, 2000));
  const status = await call(`${API}/v2/${item}:fetchStatus`, { headers: auth }, "Checking the upload");
  upload = { uploadState: status.lastAsyncUploadState ?? upload.uploadState };
}
if (upload.uploadState !== "SUCCEEDED") fail(`Upload ended in state ${upload.uploadState}: ${JSON.stringify(upload)}`);
console.log(`✓ Uploaded ${zip}`);

const published = await call(`${API}/v2/${item}:publish`, { method: "POST", headers: auth }, "Publish");
console.log(`✓ Submitted for review: ${published.state ?? JSON.stringify(published)}`);
