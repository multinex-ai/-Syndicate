---
name: external-attack-surface
description: Discover and monitor the external network perimeter of an organization you are authorized to assess — internet-facing hosts, services, exposure and known-vulnerable components. Awareness and detection, defensive. In-scope assets only.
---

# External attack surface awareness

Your external attack surface is everything an outsider can reach from the internet. This skill maps and watches that surface for **an organization you are authorized to assess**, so exposure gets noticed and closed. Guidance only; grants no tools or permissions.

## Authorization and scope

Only assets the organization owns and has authorized: its IP ranges, domains, and the services on them. Confirm ownership before probing — do not scan addresses you cannot show belong to the authorized scope. Passive discovery of public records is always safe; active probing follows the rules of engagement in `authorized-pentest`.

## What makes up the surface

1. **Names and addresses.** Domains, subdomains, certificates, the IP ranges the organization actually owns. Watch for forgotten subdomains and dangling DNS records (subdomain takeover risk).
2. **Reachable services.** What ports and services answer on in-scope hosts, and their versions. Management interfaces, databases, remote access, and dev/staging systems exposed by accident are the usual trouble.
3. **Web and API exposure.** Public apps and endpoints, their technologies, authentication posture, and anything that leaks internal detail.
4. **Third-party and cloud footprint.** Cloud buckets, SaaS tenants and CDNs tied to the organization — often exposed by misconfiguration rather than a bug.
5. **Leaked assets.** Credentials, keys or internal docs exposed in public repos or paste sites tied to the organization.

## Detection, not just a snapshot

- **Baseline** the known-good surface, then **watch for change**: a new open port, a new subdomain, a service that changed version, a certificate that appeared.
- **Correlate with advisories.** When a new vulnerability lands for a component you run (via `vuln-episodic-memory` and OSINT), check whether your exposed surface includes it, and how reachable it is.
- **Prioritize by reachability.** Remote-unauthenticated exposure of a known-vulnerable service is the top of the list.

## Discipline

- Confirm ownership before active probing; stay inside the signed scope.
- Minimum-impact probing; respect rate limits; nothing destructive.
- Findings and fixes go to the owner, routed through `vuln-remediation`.
- Record the surface and its changes in `vuln-episodic-memory` so drift is visible over time.

See [the surface inventory](references/SURFACE.md).
