# Security review — e03-target-device-restructure (main...HEAD @ f6f9410)

Scope: 88 files, +2843/−500. Rust X11 client + shell deploy scripts + specs.

## Findings

- **Secrets exposure:** none. `secrets/` and `token.json` remain gitignored; diff
  touches only `.env.debug`'s ROOT_DIR path (no credentials). Token is read from
  disk at runtime, sent as Bearer over HTTPS only.
- **Command injection (shell scripts):** none. flash.sh / flash_usb.sh use fixed
  paths and a fixed `root@kindle` scp target; the token mv-aside is path-fixed.
  The user's rename commit (bf50805) updated flash_usb.sh; flash.sh was fixed in
  verify (f6f9410).
- **Auth/OAuth:** PKCE S256 flow unchanged; scopes widened with `follow:read`
  (read-only, no new write surface beyond existing challenge:write).
- **SSRF/path traversal:** none — all API calls go to the compile-time
  `LICHESS_API_BASE` (https) or the litchee client's fixed base; challenge/game
  ids are used in URL paths via litchee's `http::segment` encoding.
- **Unsafe deserialization:** serde JSON only, all DTOs typed.

## Verdict

No HIGH findings. Gate: PASS.
