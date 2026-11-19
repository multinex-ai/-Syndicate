---
name: smart-device-security
description: Assess the security of smart and IoT devices you own or are authorized to test — firmware, default credentials, network protocols, update posture and data handling. Awareness, detection and hardening. Physical possession or written authorization required.
---

# Smart device and IoT security

Smart devices — cameras, hubs, sensors, appliances, routers, wearables — are small computers that often ship insecure and rarely get patched. This skill assesses devices **you own or are authorized to test**, so they can be hardened. Guidance only; grants no tools or permissions.

## Authorization

Only devices you physically own or have written authorization to test, on a network you control. Do not touch devices on networks that are not yours. Prefer an isolated test network segment, not production.

## What to check

1. **Credentials.** Default or weak passwords, hardcoded accounts, shared keys across units. Can it be changed, and is a change forced on setup?
2. **Network exposure.** What ports and services the device opens, on the LAN and (worryingly) to the internet via UPnP or cloud tunnels. Unauthenticated services are the classic IoT hole.
3. **Protocols.** How it talks: TLS or plaintext, certificate validation, MQTT/CoAP/Zigbee/BLE pairing and encryption. Look for cleartext credentials and commands.
4. **Firmware.** How updates arrive: signed and verified, or unsigned and spoofable? Is the current version current? Firmware often contains secrets and debug backdoors — review it on devices you own.
5. **Companion app and cloud.** The phone app and vendor cloud are part of the device. Check auth, API exposure, and whether one account can reach another's device.
6. **Data handling.** What the device collects, where it sends it, and whether that is encrypted and expected.

## Detection and hardening

- **Inventory** every smart device on the authorized network; unknown devices are findings in themselves.
- **Baseline** normal traffic per device, then watch for change — a camera suddenly talking to a new host matters.
- **Segment.** Smart devices belong on an isolated network segment away from sensitive systems; recommend and verify this.
- **Prioritize** internet-reachable, unauthenticated, or unpatchable devices.

## Discipline

- Own-or-authorized devices only, on your own network.
- Non-destructive; do not brick devices or disrupt others' service.
- Findings and hardening steps go to the owner, routed through `vuln-remediation`.
- Record devices, firmware versions and findings in `vuln-episodic-memory`; IoT vulnerabilities recur across a whole product line.

See [the device assessment](references/DEVICE.md).
