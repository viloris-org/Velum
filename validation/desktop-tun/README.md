# Desktop TUN Release Evidence

Desktop TUN remains feature-gated until one retained run exists for Windows,
Linux, and macOS from the same release candidate. A run records the signed or
packaged artifact hashes, OS build, architecture, helper protocol version, and
profile schema version.

Each platform run must cover:

1. clean install and authorization denial;
2. helper or system-extension identity rejection for an unauthorized client;
3. IPv4 and IPv6 TCP, UDP, and DNS through `PROXY`, `NODE`, `DIRECT`, and
   `REJECT` actions;
4. route and DNS partial-apply rollback;
5. UI, data-plane, and privileged-host process death at each lifecycle phase;
6. next-launch journal recovery before a new start;
7. suspend/resume, default-network replacement, and relay loss;
8. signed upgrade, downgrade rejection, stop, and uninstall with no remaining
   interface, route, DNS, service, or extension state.

Store raw logs only after applying destination and secret redaction. Packet
captures, credentials, certificates, full destinations, and secure-storage
contents are not retained. A checklist without command output and artifact
hashes is not release evidence.
