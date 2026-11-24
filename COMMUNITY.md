# Yonodex Community & Coordination

*Whitepaper Layer 10: Censorship-Resistant Coordination*

To ensure Yonodex remains operational even if centralized platforms (Discord, Telegram) are blocked or seized, we use decentralized and federated communication tools.

## 1. Matrix (Federated Chat)
We use Matrix for all public and developer communication. If one homeserver goes down, the network continues.

*   **Client:** [Element](https://element.io/)
*   **Homeserver:** `matrix.org` (with plans to self-host `matrix.yonodex.org` in Phase 5)
*   **Official Rooms:**
    *   `#general:yonodex.org` (Public community chat)
    *   `#dev:yonodex.org` (Core development & PR discussions)
    *   `#trading:yonodex.org` (Market making and trading strategies)
    *   `#governance:yonodex.org` (DAO proposal discussions)
    *   `#security:yonodex.org` (Invite-only for security researchers)

## 2. Discourse (Governance Forum)
Long-form governance proposals and debates happen on our federated forum. We back up all proposals to IPFS.

*   **URL:** `forum.yonodex.org` (Planned Phase 5)
*   **Structure:**
    *   **Proposals:** Formal YIPs (Yonodex Improvement Proposals).
    *   **Temperature Check:** Community sentiment before formal voting.
    *   **Audit Reports:** Public repository of Slither, Certora, and Trail of Bits reports.

## 3. Signal (Security & Core Dev)
For high-security coordination (bug bounties, emergency patches, Guardian multisig communication).

*   **Security Disclosures:** Email `security@yonodex.org` (PGP encrypted).
*   **Core Dev Chat:** Signal group with disappearing messages (24 hours). No metadata logging.

## 4. Waku / Status (Fallback)
In the event of a total internet crackdown, core developers use the Status app (Waku P2P messaging protocol) to coordinate. This runs entirely over libp2p, just like Yonodex.

---
*Last Updated: November 2026*