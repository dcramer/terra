# Character controller: Jolt's CharacterVirtual and Godot's CharacterBody3D

2026-10-24. The question: whose character controller do we port for every body's mover (movement.md layer 2, D53), and what does each do that ours does not? Today's walker (`terra_walker/src/movement.rs`, `bodies.rs`) is a point at the feet snapped to the ground SDF, head probes, and a 0.3 m circle on the ground. It has no capsule and no swept collision. Water past 0.7 m is a wall, and step heights disagree between sources (0.7 m on terrain, 0.4 m on props) and pop. Its pushes run once in order, so a later one can undo an earlier, and other bodies never push the player.

## Answer

Port Jolt's `CharacterVirtual` (Jolt 5.6.0, 2026-07-11, MIT) for the move itself: collide-and-slide under contact constraints, stair walking, stick-to-floor, the slope limit and the ground states. It runs over our own collision queries, since our ground is an SDF and not a Jolt shape. Take three of Godot 4.7.2's `CharacterBody3D` (MIT) defaults that Jolt lacks: a floor that keeps its speed, a minimum wall-slide angle and a stop on slopes. Keep our own mass-weighted separation between bodies, because Jolt does not weigh one character against another.

## Jolt CharacterVirtual (5.6.0)

`Jolt/Physics/Character/CharacterVirtual.{h,cpp}` and `CharacterBase.h` at tag v5.6.0. Each `Update(dt, gravity)` runs these steps:

1. **Contacts at the start** (`GetContactsAtPosition`). The shape is inflated by `mPredictiveContactDistance` (0.1 m) and every contact within that distance is collected: position, normal, distance (negative when in penetration) and the other body's velocity. Opposing contacts are pruned (`RemoveConflictingContacts`).
2. **Contacts become planes** (`DetermineConstraints`). Each contact is a plane the velocity may not cross. A penetrating one adds a push-out velocity of `-normal * distance * mPenetrationRecoverySpeed / dt` (recovery 1.0: all of it in one update). A contact steeper than `mMaxSlopeAngle` (50 deg) gets a second, vertical plane, so a steep slope is a wall to walk into but not a ramp.
3. **The solve** (`SolveConstraints`, at most `mMaxConstraintIterations` 15). The velocity is projected onto the planes it hits first, by time of impact. When two planes hold it, it slides along the crease, and three stop it. A hit against a dynamic body gives that body an impulse clamped by `mMaxStrength` (100 N), and gravity on what the character stands on comes from `mMass` (70 kg).
4. **The move** (`MoveShape`, at most `mMaxCollisionIterations` 5). The shape is moved along the solved velocity and stops at the first contact a sweep finds (`GetFirstContactForSweep`). It keeps `mCharacterPadding` (2 cm) off geometry and accepts `mCollisionTolerance` (1 mm) of penetration, and it stops when less than `mMinTimeRemaining` (1e-4 s) is left.
5. **Ground state** (`UpdateSupportingContact`). One of `OnGround`, `OnSteepGround`, `NotSupported` or `InAir`. The supporting contact is any whose normal is within the slope limit and lies behind `mSupportingVolume`. By default that plane accepts every contact, and Jolt's samples set it through the centre of the capsule's lower sphere. The ground's velocity is carried, which is how a character stands on a platform.

`ExtendedUpdate` wraps that with two passes, and its defaults are the industry's own:

- **WalkStairs** runs when the move was blocked by a steep contact while walking forward (`CanWalkStairs`, the contact within 75 deg of the move). It moves up `mWalkStairsStepUp` (0.4 m), forward by the remaining move (at least 0.02 m), and down again. It keeps the result only if it lands on walkable ground and a test 0.15 m further forward (`mWalkStairsStepForwardTest`) finds the floor there too, so the edge of a slope is not taken for a step. The step is instant: smoothing what is shown is left to the game.
- **StickToFloor** runs when a body that was on the ground last update is now in the air and not moving up. It sweeps down up to 0.5 m (`mStickToFloorStepDown`) and snaps onto walkable ground if it finds some. This is what keeps a sprint down stairs or a hill grounded.
- `mUp` can be set per character and per update (`SetUp`). Gravity toward a planet's centre needs nothing more.
- **Character against character** (`CharacterVsCharacterCollision`, added in 5.1) treats the other character as a moving obstacle: its shape and its velocity, padding summed. Mass plays no part, so neither gives way. An optional **inner body** (a kinematic rigid body that follows the character) lets ordinary rigid bodies, rays and sensors find it.
- **Determinism**: 4.0.2 made the order of active contacts deterministic, and 5.3 gave every character a `CharacterID` that orders character-against-character contacts, so a client and a server get the same result.

Changes to CharacterVirtual since 4.0, from `Docs/ReleaseNotes.md`:

- 5.6 fixed a character stuck between slopes steeper than the limit after a teleport, where the solver ping-ponged between two planes. It also fixed sliding along a wall low enough to start a stair walk but too high to finish one, which sped the character up, and padding not always being kept.
- 5.5 added compound shapes as the character's shape.
- 5.3 added `CharacterID`, inner-body override and persisted-contact callbacks.
- 5.2 added ScaledShape.
- 5.1 added character-against-character collision, the inner body, and `mEnhancedInternalEdgeRemoval`, which stops a body catching on the internal edges of triangle meshes.
- 4.0 has fixes worth knowing when we port. A character was reported on the ground for one frame after jumping against a wall. It sped up sliding along a wall. It jittered moving down on an elevator. A delta-time term was missing in `DetermineConstraints`.

## Godot 4.7.2 CharacterBody3D

`scene/3d/physics/character_body_3d.{h,cpp}` at 4.7-stable. `move_and_slide` does up to `max_slides` (6) motion tests, each a sweep with `safe_margin` (1 mm). It sorts each hit into floor, wall or ceiling by its angle to `up_direction`, with `floor_max_angle` at 45 deg. It has no stair walking, so games add it. What Jolt lacks, and we take:

- **`floor_constant_speed`** (off by default): speed along a slope equals the speed on the flat, rather than the flat speed projected onto the slope. We want it on: a slope's slowdown is our own curve on top of it, not geometry.
- **`floor_stop_on_slope`** (on): standing still on a walkable slope does not slide down.
- **`floor_block_on_wall`** (on): a body is not lifted up a wall it pushes into.
- **`wall_min_slide_angle`** (15 deg): a wall met nearly head-on stops the body instead of sliding it sideways. This is what keeps a body from creeping along a wall while pushing into it.
- **`floor_snap_length`** (0.1 m) is the same as Jolt's stick-to-floor.
- **`platform_on_leave`**: when a body steps off a moving platform it keeps the platform's velocity.

## Others considered

- **Rapier's `KinematicCharacterController`** (Apache-2.0, plain Rust) has autostep, snap-to-ground and climb and slide angles. It answers one sweep at a time through Rapier's query pipeline, with no contact-plane solver, no character-against-character contacts and no ground states. We would still have to wrap our SDF as a Rapier shape.
- **bevy_tnua and Avian's character examples** depend on Bevy and on Avian's colliders. The server runs the mover without Bevy (D28, D447), so neither fits.
- **Unreal's Character Movement** is documentation only (D196). Its step-up, perching on ledges, mesh smoothing of steps on simulated proxies, and network prediction with replay are the model for presentation and netcode. Its code is not ours to take.

## What we port

- **From Jolt** (MIT, the notice kept in the ported module, D196): the steps above, plus WalkStairs and StickToFloor and the ground states. The queries are ours. "Contacts within a distance of a capsule" and "first contact of a swept capsule" are asked of each collision source, and the SDF answers by its distance and gradient. A sweep over an SDF moves the capsule forward by its distance to the ground divided by the field's steepest slope (`Field::steepest`), as sphere tracing does, so it cannot pass through a voxel-thin wall at any speed.
- **From Godot**: constant floor speed, stop on slope, block on wall and the minimum wall-slide angle, as settings.
- **Ours**: separating bodies by mass after every mover has run, solved over a few rounds together rather than one push after another. Jolt neither weighs characters against each other nor fixes an order for them. Water is treated as a medium, and stepping up is smoothed on screen.
