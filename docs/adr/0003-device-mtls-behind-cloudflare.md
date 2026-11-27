# ADR 0003: Device Authentication with mTLS at the Cloudflare Edge

Without this ADR, a competent agent would authenticate the device with a bearer token checked in Axum, or would try to terminate mTLS in Axum itself, where the client certificate never arrives because Cloudflare terminates TLS at its edge.

## Status

Accepted

Prerequisites:

- **Cloudflare, verified on 2026-11-27** with a test certificate and hostname: a client certificate from the Cloudflare-managed CA can be issued, mTLS can be enforced on a hostname, the origin receives the certificate SHA-256 fingerprint and it matches the certificate, and a revoked certificate is blocked at the edge. Server implementation is unblocked.
- **Hardware-backed Keystore on the XCover 5: not verified yet.** It blocks certificate enrolment in the locator, not the server.

## Context

The server is reachable only through a Cloudflare Tunnel. Cloudflare terminates TLS at its edge, so a client certificate presented by the device never reaches Axum. The original design authenticated the device with a per-device bearer token plus a Cloudflare Access service token as a second layer. Both are reusable secrets sent on every request.

Only one device is managed for now.

## Decision

Authenticate the locator with a client certificate validated by Cloudflare, and identify the device at the server by the certificate fingerprint.

- **Hostnames**: the device API and WebSocket live on an API hostname that requires mTLS. The web UI lives on a separate hostname behind an interactive login, because a browser has no client certificate.
- **Edge enforcement**: mTLS is enabled on the API hostname, with a WAF rule that blocks any request whose client certificate is not verified or is revoked.
- **Device identity**: Cloudflare forwards the certificate SHA-256 fingerprint to the origin in a request header, `X-Client-Cert-Sha256`, set by a request header transform rule from `cf.tls_client_auth.cert_fingerprint_sha256`. The value is lowercase hex without separators. The server stores one fingerprint per device and rejects requests whose fingerprint does not match the `{device}` in the path. This replaces both the bearer token and the Access service token for device traffic.
- **Trust in the header**: the server listens on localhost only and is reachable only through the tunnel, so the forwarded header cannot be set by anyone but Cloudflare.
- **Private key**: generated on the device in Android Keystore as non-exportable. It never leaves the device and is never stored in Git.
- **Agent**: the root agent does not hold the device key. Anything it needs to send goes through the locator.

## Enrolment

The certificate is created when the locator is deployed to the device. It is a manual, operator-driven step over `adb`:

1. The locator generates the key pair in Android Keystore and produces a CSR.
2. The operator pulls the CSR and has it signed by the Cloudflare-managed client CA for the zone.
3. The operator pushes the signed certificate back to the locator.
4. The operator registers the certificate fingerprint for the device in the server.

No self-service enrolment endpoint exists. Adding one is deferred until more than one device is managed.

## Consequences

- No reusable secret travels with requests, and unauthenticated traffic is dropped at the edge before it reaches the server.
- Cloudflare still sees request contents in clear text. This is not end-to-end mTLS to Axum; that would require exposing the origin directly or a paid TCP passthrough, both rejected.
- Root on the device can use the key but cannot extract it if the Keystore is hardware-backed. Whether it still is on the XCover 5 with Knox tripped must be verified on the device.
- An expired or wrongly revoked certificate silences the device, including SOS. The certificate gets a long validity, the server tracks its expiry date and warns ahead of it, and silence detection remains the safety net.
- Replacing the device, or rotating the certificate, means repeating the enrolment by hand.
- The edge rule must block both an unverified and a revoked certificate; a revoked certificate is not caught by the verification check alone.
