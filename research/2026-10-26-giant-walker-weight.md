# Selling a slow giant's weight: what shipped work does, and the Cetri's walk measured against it

2026-10-26. Question: the Cetri (a 7 m walking oak, D534) scored 3 of 10 for animation weight in its round-7 film. How do games and films make a slow giant walker feel heavy, as numbers we can measure on our clip, and where does ours fall short?

## Answer

Weight comes from timing that is keyed to the feet: each footfall's dust, camera shake and settle land on the clip's contact frame, and the steps take as long as a body that size takes. Round 7 missed this. Its dust, shake and trunk rock were struck every 1.0 s, but the feet landed every 1.54 s. So the beats fell off the feet, and the feet slid 29% against the ground. The fix keys every beat to the measured footfalls. It sets the body's speed to the clip's own pace at its playback rate, and holds the trunk briefly at the bottom of each step.

## What shipped work does

- **Size slows the start and stop, not the top speed.** Masanobu Tanaka was animation director on Shadow of the Colossus and The Last Guardian. In his CEDEC 2021 talk, "5 Rules for Animating Giant Characters", he says a large mass cannot start suddenly and always follows a smooth motion curve, though it is not limited in top speed ([gameanim.com](https://www.gameanim.com/?p=76076)).
- **Time to build up and shed speed; a stop ripples out slowly.** David Clayton, Weta Digital, on Rampage ([VFX Voice](https://www.vfxvoice.com/weta-digital-gives-weight-to-the-giant-monsters-of-rampage/)). For Godzilla: King of the Monsters, MPC's Spencer Cook asked for extra anticipation and extra follow-through ([befores & afters](https://beforesandafters.com/2024/05/01/for-godzillas-walk-we-put-a-speedometer-on-his-feet-just-to-see-how-fast-they-go/)).
- **Camera shake accents the weight; it does not carry it alone** (animators' consensus, e.g. [polycount](https://polycount.com/discussion/comment/1423704)). Engines play footstep shakes and effects from an animation event on the clip's contact frame, Unreal's Anim Notify being one. The shake's size falls off with distance from the epicentre, as in Unreal's world camera shake with its inner and outer radius.
- **Step time scales with the square root of leg length.** This is dynamic similarity at equal Froude number v²/(g·h), from Alexander's inverted-pendulum model ([PMC3798186](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3798186/)). The Cetri's hip is 2.9 m high and a person's 0.9 m, so its steps take about 1.8 times as long as a person's. A person's slow, heavy step of about 0.8 s becomes about 1.45 s.
- **The weight sits in the down pose** (classic walk animation): the body is lowest just after contact. A heavier body reaches that point later and stays there longer. The crown and limbs then overlap: they carry on after the trunk stops, and spring back past it.

## The Cetri's clip, measured

Troll_Walk on `assets/models/characters/cetri.glb`, read with `tools/characters/glb.ts`:

| Measure | Round 7 | Round 8, as scripted | Aim |
|---|---|---|---|
| Clip cycle; the pace its file keeps | 1.542 s; 2.254 m/s | same | |
| Played at (pace rate, clamped at 0.5) | 0.5 | 0.5 | |
| Body speed | 0.8 m/s, so the feet slide 29% | 1.127 m/s, the pace × 0.5: no slide | no slide |
| Step period on screen | 1.43-1.65 s | 1.53-1.77 s, with the settle holds | 1.2-1.6 s (Froude) |
| Footfall beats (dust, shake, rock) | every 1.0 s, off the feet by 0.1-0.65 s | on each contact frame | within a frame |
| Pelvis drop at contact (clip) | 0.127 m, lowest 0.05-0.09 s after contact | same, then held at 0.3 speed for 0.16 s | lowest after contact, held |
| Crown after the trunk | the head 0.07 s behind the pelvis in the clip; no overshoot | nods 7 degrees 0.18 s after contact, springs back 2.5 degrees at 0.5 s | lags, then overshoots |
| Camera shake | a 5-8 cm drop over 5 frames, off the feet | 8 cm at 4.5 m (0.7-1.6 times by distance), a 5-part decaying bounce over 22 frames, pitch 1.6 times as far | on contact, by distance |
| Face from the walker, 12 s to 16.5 s | about 4.7 m to 3.7 m (1.3 times as large) | 5.1 m to 3.5 m (1.46 times) | 1.5 times |

The same file shows why the face went dark in the lunge. The eyes and mouth ride `neck_01`. Tipped with the trunk, the face pointed 88 degrees below level, and the lens was 72 degrees off its line, so the face read as a slit. Held up against the tip, the face points 37 degrees down and the lens is 8 degrees off its line. The scene's constants say the rest (`tools/sandbox/trailer/scenes/cetri.ts`).
