# Simulated Multiplayer Avatars

The first multiplayer presentation layer uses remote-avatar snapshots. The game
can display named ghost avatars and interpolate them smoothly toward received
positions. This is a visual/client-side simulation only: it does not connect to
other computers, synchronize worlds, or authenticate remote users.

A real multiplayer release needs a server protocol, identity/session handling,
authoritative movement validation, world-delta synchronization, rate limits,
and disconnect handling. The local account file is not an online identity.
