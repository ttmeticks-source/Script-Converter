# Optional Pumpkin server direction

Pumpkin is an independent, high-performance Rust Minecraft server. It is a
server process, not a client renderer or a graphics library, so adding it as a
Cargo dependency would not speed up RealismCraft's block drawing.

## Why it is not linked into the game binary

RealismCraft currently uses its own local world-save format and local account
profile. Pumpkin speaks Minecraft's network protocol and maintains Minecraft
server world state. The two formats and protocols are not automatically
compatible. A bridge must explicitly translate player sessions, block IDs,
world changes, chat and movement before RealismCraft can join a Pumpkin server.

Pumpkin Server is GPL-3.0. Its plugin API crates are separately dual-licensed
under MIT OR Apache-2.0. Do not copy Pumpkin server source into this MIT project
or link its server implementation into a distributed game build without
reviewing the GPLv3 obligations. The safest integration boundary is a separate
optional server process plus a documented protocol adapter.

## Planned adapter boundary

1. Add a RealismCraft multiplayer protocol and authenticated player sessions.
2. Implement a bridge that maps supported RealismCraft blocks and player
   snapshots to the server's protocol/data model.
3. Keep RealismCraft-only materials and world-generation features behind
   capability checks so unsupported blocks are never silently corrupted.
4. Add server discovery, disconnect handling, rate limits and world backups.
5. Verify interoperability in an isolated test world before enabling public
   servers.

Status: architecture note only. RealismCraft does not currently connect to
Pumpkin, and the current ghost-avatar layer is only a visual simulation.
