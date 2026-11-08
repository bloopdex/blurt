# Chrome Web Store listing: Send to Blurt

Paste these into the developer dashboard when creating the item. The images are in this folder, and the package is `blurt-extension.zip` from the `ext-v*` GitHub release (or the Extension workflow's artifact).

## Store listing

- **Name:** Send to Blurt
- **Summary (≤132 chars):** Send a YouTube, TikTok or Instagram video to the Blurt soundboard in one click, ready to trim and play into your mic.
- **Category:** Tools
- **Language:** English
- **Icon:** `extension/icons/128.png`
- **Screenshot:** `screenshot-1280x800.png`
- **Small promo tile:** `promo-440x280.png`
- **Homepage:** https://blurt.bloopdex.com/
- **Support:** https://github.com/bloopdex/blurt/issues

**Description:**

> Send to Blurt is the companion for Blurt, the free desktop soundboard that plays sounds straight into your microphone for Discord, games, Zoom and streams.
>
> Found the perfect clip? Click Send to Blurt (or right-click a video or link, or press Alt+Shift+B). Blurt fetches the audio, and you trim it and add it to your board.
>
> • Works with YouTube, TikTok, Instagram, X, SoundCloud, Reddit, Twitch and Vimeo
> • Talks only to the Blurt app on your own computer (127.0.0.1). No accounts, no tracking, no remote servers
> • Requires the Blurt desktop app for Windows: https://blurt.bloopdex.com/
>
> Only send content you have the right to use. Blurt is open source: https://github.com/bloopdex/blurt

## Privacy tab

- **Single purpose:** Send the address of the video page you choose to the Blurt desktop app running on the same computer, so Blurt can add its audio to your soundboard.
- **Permission justifications:**
  - `activeTab`: read the current tab's URL and title only when the user clicks the toolbar button or presses the shortcut, to send it to Blurt.
  - `contextMenus`: add "Send this video to Blurt" / "Send link to Blurt" to the right-click menu on supported sites.
  - Host permission `http://127.0.0.1:7270/*`: reach the Blurt desktop app on the user's own computer. No other host is contacted.
- **Remote code:** No, I am not using remote code.
- **Data usage:** The extension collects none of the listed data types. The URL it sends goes only to the user's own computer, never to the developer or a third party.
- **Certifications:** tick all three (not sold, not used for unrelated purposes, not used for creditworthiness).
- **Privacy policy URL:** https://blurt.bloopdex.com/privacy.html

## Review risk

The Chrome Web Store has rejected extensions whose purpose is downloading YouTube videos. This extension doesn't download anything itself: it hands a URL to a separate desktop app. A reviewer may still apply that policy. If it's rejected, the zip can still be installed with "Load unpacked", and the same package can be submitted to Microsoft Edge Add-ons.
