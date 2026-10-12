# Spike: one shared mesh for every body

Question: can every body be a shape of one base body asset (MakeHuman's CC0 mesh with our skeleton), looking as good as our Tripo bodies, with gear made once fitting every shape? The plan it tests is [docs/character-system-plan.md](../../docs/character-system-plan.md); findings go in [research/one-mesh-for-every-body.md](../../research/one-mesh-for-every-body.md).

`fetch.sh` fetches MakeHuman's data into `mh/` (ignored). `mh.py` reads its formats (no MakeHuman code, which is GPL). `fit.py` fitted a body to one of ours, `paint.py` takes Tripo's painting of it back, `compare.py` renders bodies side by side; outputs go to `out/` (ignored).

So far (2026-10-12): the shape fit comes within about 4 mm, Tripo's paint on the shared mesh looks like the originals, and the face holds only when fitted by face shapes. The rig checks fail where weights and joints were borrowed from each body's Tripo rig; the plan replaces that with a base asset made once.
