# Surface inventory

A living record of the authorized external surface, kept so change is visible.

```
asset:        domain / subdomain / host / IP / bucket / endpoint
ownership:    how it was confirmed in-scope (record, authorization pointer)
first_seen:   when it entered the baseline
service:      what answers here, and version if known
exposure:     internet-reachable? auth required? management/dev surface?
known_vulns:  advisories that apply to this component (link to episodic memory)
reachability: remote-unauth > remote-auth > restricted
change:       appeared / disappeared / version changed / cert changed
action:       monitor | investigate | remediate (route to vuln-remediation)
```

Watch items that matter most:

- New or forgotten subdomains and dangling DNS (takeover risk).
- Management interfaces, databases, remote access exposed to the internet.
- Dev/staging systems reachable from outside.
- Cloud storage or SaaS misconfigured to public.
- A component you run gaining a new known vulnerability.
