# Device assessment

Per authorized device.

```
device:        make / model / role
ownership:     owned | authorized (pointer)
network:       segment it sits on; should be isolated from sensitive systems
firmware:      current version; latest available; signed-update? yes/no
credentials:   default changed? forced on setup? shared across units?
lan_services:  open ports/services on the local network
wan_exposure:  reachable from internet? via UPnP / cloud tunnel? auth?
protocols:     transport encryption, cert validation, pairing security
app_cloud:     companion app + vendor cloud auth and isolation
data:          what it collects, where it sends it, encrypted?
findings:      issues, each routed to vuln-remediation
hardening:     change creds, disable UPnP, segment, update, disable cloud if unused
```

Red flags:

- Internet-reachable with default or no authentication.
- Unsigned firmware updates, or no updates available.
- Plaintext protocols carrying credentials or commands.
- One account able to reach another customer's device.
- Device on the same flat network as sensitive systems.
