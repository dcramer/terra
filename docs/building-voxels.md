# Building voxels: the contract

What a construction cube is, who draws each face, edge and corner between cubes, the invariants that keep a building closed, and every junction with the rule that closes it and the check that proves it. Decided as the voxel contract (D348): a change to how cubes are laid or drawn keeps these invariants, or changes this page in the same commit. Where a junction has no rule or no check, it is listed under [Gaps](#gaps).

The developers (2026-10-15), of walls and roofs: "we should implement it correct, but the issue is we had gaps, because we didnt think this through"; and "i think its worthwhile to have a very clear design doc on how the voxels need to work so we can make sure there's no gaps?"

It rests on D181 (strictly on the grid), D192 to D194 ([construction-001](proposals/construction-001.md)), D197, D215, D231, D241, D290 ([construction-002](proposals/construction-002.md)) and D343. The worked cases are the baseline (D333): logs, hewn timber and shakes; stone and slate. No rule here is a material's: every rule is a pattern's, so it holds for any material of that pattern.

## 1. The cube

A cube is half a metre on its base's own grid (D57, D181), stored in sections of 16 a side, 32 bits each (`terra_build::cubes`):

| Bits | What | Values |
|---|---|---|
| Kind (12) | Its material, or air | A material file's id; 2048 and up are roof kinds (`Kind::is_roof`). Its pattern comes from the material: **block** (stone; code `Masonry`), **coursed** (logs, hewn; code `Logs`), **frame** (a block with `framing`), **roof** (shakes, slate) |
| Shape (5) | What part of the cube it fills | The table below; 31 is an opening (`Fill::Open`) |
| Fill (4) and built (1) | How far in | Sixteenths; a coursed cube goes in by courses, a block cube at once, a roof cube in four stages (frame, battens, covering, trims) |
| Pending (2) | A change marked on it | None, carve, replace, swapped (D232) |
| Part (8) | The plan it was laid in | An index into its section's palette (D260) |

Faces are named by the side a shape rises toward (`Facing`, north here). **Full**: the shape covers that square face whole, so a neighbour's face against it may be hidden. **Part**: it covers some of it, and the rest is open. **None**: it reaches that face only along an edge or not at all.

| Codes | Shape | Occupies | Top, bottom | Faces N, S, E/W |
|---|---|---|---|---|
| 0 | Whole | All of it | Full, full | Full, full, full |
| 1 | Half | Its lower half | None (a planed top at 0.5), full | Part (lower half) on all four |
| 2 to 5 | Stair | A lower step across it, an upper step at the side it rises to | Part (two treads), full | Full; part (lower half); part (an L) |
| 6 to 9 | Slope | A ramp up to the side | None (the ramp), full | Full; none; part (a triangle) |
| 10 | Post | An upright as narrow as its kit dresses `Role::Post`; holds and supports as a whole cube; a walker meets a 36 cm box | Carries what is on it, full | None for drawing: its shaft shows all round |
| 11 | Upper half | Its upper half | Full, none (a planed underside) | Part (upper half) |
| 12 to 15 | Stair upside down | The side's half whole, the upper half of the rest | Full, part | Full; part (upper half); part (an L) |
| 16 to 19 | Slope upside down | Whole at the side, its underside a ramp | Full, none (the ramp) | Full; none; part (a triangle) |
| 20 to 23 | Outer corner slope | The lower of two ramps, toward the side and the next clockwise: a hip's | None, full | Part (triangles) toward the two sides; none away |
| 24 to 27 | Inner corner slope | The higher of the two: a valley's | None, full | Full toward the two sides; part (triangles) away |
| 28, 29 | Unused | | | |
| 30, 31 | Opening | Nothing; still wall, so a room stays closed; a walker passes | Open | Open (30 was glass before D225) |

Every shape has a top and an underside over each point (`Shape::top`, `Shape::bottom`), which is what a walker stands on and passes under. Support, cost, order and taking apart treat every shape as a whole cube (D220, D231). A cube is planned (a ghost, walked through), in to a fill level (collides to its fill) or built.

**What hides a face** (`sections::draw_blocks`, its `hides`): only a cube that covers it whole. A block kind's face is hidden by its own kind's whole cubes, by a floor or footing of another kind and by the plain kind; never by another kind's wall, which is drawn thinner than its cube, nor by a roof's cube, which is drawn as a roof over it. A cut cube's surface is hidden by its own kind's whole cubes beside it, but not by one on its cut top or under its cut underside. A face against an opening, the air or the ground shows.

## 2. Who owns what

Every face, edge, corner and piece belongs to exactly one cube, line or prop, so it is drawn once and goes in, comes out or is carved with its owner. None is stored: all are worked out from the cubes, the ground's samples, the kits and the base's seed when a section is laid.

| What | Owner | Rule |
|---|---|---|
| A cube's mass | The cube | Its kit's pattern draws it: a block surface, courses of members, a deck, a roof square |
| A face between two cubes | The cube whose face it is, if it shows (§1); nobody between two whole cubes of one kind | Two kinds each draw their own face up to the boundary; faces back to back are never seen and not counted |
| An exposed edge (D215) | The cube whose side face it runs along | An inside corner belongs to the cube whose open face looks east or west; a border to the cube standing on the floor; an opening's head and sill to the opening, one lintel and one sill across its full width (D236); a cut cube's lines to that cube, as bands (`Piece::Band`) |
| An edge's ends, the corners | The edge | Each end is one of seven: runs on, mitred out, mitred back, steps with the ground, stops square, or bends over or under a ramp's coping (`roles::End`), so no end is left undefined |
| A joint between two kinds (D232) | The heavier, by the materials' `meets` weights | The heavier runs through and is dressed as if the lighter were not there; the lighter ends against it with its end trim (`Board`); the heavier carries its cap (`Cap`) where the lighter stands on it; two as heavy each end with their trim |
| A wall under a roof cube | The wall | Carried up into the roof cube to the deck (`Piece::Gable`, timber courses cut to the slope); the roof cube is that wall to its roles, so its steps take no dress (D241) |
| A roof cube's top | The roof | A square of covering every half cube; a side shared with the next square is drawn once, from the square to its west or north; a side that does not carry on is exactly one of eave, verge or flashing |
| A roof's frame | The roof | Rafters, beams and battens only over the roof's own cubes, stopping at any wall's face (`roofs::framed`) |
| A door's frame | The door (D225) | Where a door is fitted the kit lays no frame, post or lintel of its own (`layout::framed`) |
| A piece in the drawn sections | The section of the cube its middle is in (`Piece::anchor`, D213) | A section is cut from the base laid whole, so a piece crossing a section edge is laid once |

The roof–wall design (building-plan.md, [Where roofs and walls meet](building-plan.md#where-roofs-and-walls-meet-d343)) adds two owners: what stands on a roof cube owns its carry-down to the deck, and the roof owns the covering, boards and frame running on under it.

## 3. Invariants

Each is a property every arrangement of cubes keeps, in every material, at every detail and in every state.

1. **The cubes are the truth.** Everything drawn, collided with and counted is a pure function of the cubes, the ground's samples, the kits and the seed. Laid twice, it is the same (D192 rule 1, D208).
2. **One owner.** Every face, edge, corner and piece has exactly one owner (§2), so nothing is drawn twice and nothing is drawn by nobody.
3. **Closed.** Every face of a solid cube that shows is covered by exactly one drawn surface, and every junction between two neighbouring cubes, whatever their kinds, shapes and states, is closed by a defined piece or rule: a row of [the junction matrix](#4-the-junction-matrix). No line of sky, no slot into a room.
4. **An eighth of a cube.** Nothing drawn leaves its cubes by more than an eighth of a cube (6.25 cm), and nothing outside them collides; the only exceptions are a roof's eaves and edge pieces as modelled, footings into the ground, and the roof's covering tucked into what stands beside it (D192; the roof–wall design's R3).
5. **Sections agree.** A section laid alone is exactly the base laid whole there; a block surface is worked a cube past its section's edge, so neighbours meet with no seam (D213, D238).
6. **Every detail closes.** Near, mid, far and the hull are coarser drawings of the same closed shape: what is closed near is closed far (D278).
7. **Every state closes.** A plan, a building going up, a change of material, a carving, damage and what is left after a collapse are drawn by the same rules as a finished building: what is in is closed on its own (built hides planned, never the reverse), every role of what is in is worked out from what is built, never from the plan (D358), so a wall built halfway has a top, an end and a corner where it stops and they go as it is built on, a cube going in counting as built as far as it is filled; plans are ghosts drawn from the plan; and a roof closes stage by stage.
8. **A material adds no junction.** Junction rules belong to the four patterns; a material is data over its pattern (D290) and can change only numbers, textures, pieces of a stated size, and its `meets` weight and trims from a closed set (§5).
9. **What cannot close is refused.** A placement that would leave something unjoined, floating, built partly under something already in, or (D343) something other than a roof on a roof, is refused with its reason, never drawn wrong.
10. **Every row is proved.** Each row of the matrix names the test or inspected case that proves it; a row without one is a gap.

The roof–wall design's seven invariants (R1 to R7, [building-plan.md](building-plan.md#where-roofs-and-walls-meet-d343)) are this contract for where roofs and walls meet: R1 the roof drawn the same whatever stands on it; R2 what stands on a roof cube carried down to the deck in its own kind; R3 the covering tucked 15 cm into whatever stands beside it; R4 every covering line flashed by an apron, step or gutter piece; R5 the boards under the deck; R6 the hull drawing a cut roof cube whole; R7 nothing refused for meeting a roof. R7 contradicts D343 as written, which still refuses a wall on a roof's cube: the developers have said walls may stand on roof cubes ("but they need to be modular then so theres no gaps"), and D343's row is theirs to amend before R7 is built.

## 4. The junction matrix

Status: **Closed** (a rule and a check), **Designed** (the roof–wall design's rule, not yet built), or **Gap n** ([Gaps](#gaps)). "Probed" means the case was put through the mesh checker (`terra_build::inspect`) for this page, in logs, hewn and stone under their roofs, on 2026-10-15 at 6ae59f68; a probe is not a check, so a junction proved only by a probe is still a gap. Test names are in `crates/terra_build`, under `src/` unless a path says otherwise.

### Walls with walls

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| Side by side, in a run | Block: faces between culled, one surface. Coursed: a log a course along the run | `surface`: `cubes_side_by_side_are_one_surface_with_no_face_between`; `runs`: `a_room_has_a_log_a_course_on_every_wall_and_stays_inside_its_cubes` | Closed |
| A free end | Arris up it; logs cut flush, every other course a hair short | `roles`: `a_free_wall_has_arrises_up_its_ends_crests_along_its_top_and_a_foot`; `runs`: `a_free_end_is_cut_flush_every_other_course_a_hair_short` | Closed |
| L, outside and inside | Arris and Inside, crests mitred out and back; logs interlock by axis, half a course apart | `roles`: `an_l_has_an_inside_corner_whose_crests_mitre_back_into_it`; `runs`: `walls_either_way_never_share_a_course` | Closed |
| T and cross | The through wall runs on, no joint where another crosses | `runs`: `no_joint_falls_where_another_wall_crosses`; the checker's T and U plans | Closed |
| Walls of different heights; a wall's top | A crest where nothing is over it | `roles`: `a_free_wall_has_arrises…` | Closed |
| A wall two or three thick, a pier | Block: one mass, an opening's head sunk behind each face's lintel through its cube. Coursed: skins along the faces, crossing only at the ends (D263); a solid mass's top a deck whose rims meet at its corners, the one along the joists' ends running through and the other butting it | `runs`: `a_thick_wall_shows_logs_along_its_faces_and_sawn_ends_only_at_its_ends`; `sections`: `every_junction_case_is_drawn_without_a_fault` (a thick wall with a window); the junctions family | Closed |
| A post in a wall, or alone | A column alone is a post: one upright; a wall's logs stop at it | `runs`: `a_column_of_cubes_alone_is_a_post`; `shapes`: `a_post_is_one_upright_under_a_bolster_and_a_half_a_low_deck` | Closed |
| Two cubes of a kind meeting only along an edge | Neither dresses the shared edge; each turns round its own outside corner there, its part in the wall mitred out and its part out of the face cut at the plane halfway between the two (`roles::End::Shared`) | `sections`: `cubes_of_a_kind_meeting_only_along_an_edge_each_dress_their_own_side`; `sections`: `every_junction_case_is_drawn_without_a_fault` (cubes meeting along an upright and a level edge) | Closed |
| Every edge with a role | Dressed once, by its owner | `roles`: `every_edge_with_a_role_is_dressed_by_one_cube_only` (added with this page) | Closed |

### Kinds meeting

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| Heavier and lighter, in an L, a T or in line | The heavier runs through, quoined; the lighter ends with its end board | `trims`: `logs_ending_against_stone_have_an_end_board_up_the_joint`, `a_stone_wall_ending_against_a_log_walls_side_is_met_by_a_board`; `roles`: `how_kinds_meet_is_the_kits_table_not_their_pattern`; the checker's every two kinds mixed under every roof and plan | Closed |
| Lighter on heavier (logs on a stone plinth) | The heavier's cap (`Meet`) | `roles`: `logs_on_stone_meet_it_with_a_cap_and_stone_quoined_beside_logs`; `blocks`: `a_thinner_wall_of_another_kind_on_stone_meets_it_with_a_cap` | Closed |
| Heavier on lighter | Its foot does not sink over the lighter | `roles`: `stone_over_logs_changed_into_a_wall_has_a_foot_that_does_not_sink_over_them` | Closed |
| As heavy as each other (logs and hewn) | Each ends with its own end board | The checker's hewn-with-logs bases | Closed |
| A wall on another kind's footing | One plinth, the footing's | `roles`: `a_wall_with_a_plinth_on_a_footing_with_its_own_takes_none` | Closed |

### Openings and fitted props

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| A hole in a coursed wall | Courses end at it, framed with jambs, head and sill boards | `trims`: `a_door_cuts_the_courses_it_crosses_and_is_framed`, `a_window_has_a_sill_and_a_cube_carved_out_is_framed_too` | Closed |
| A hole in a block wall | Jamb stones, one lintel and one sill across its full width, the wall sunk behind them | `blocks`: `a_window_in_stone_has_a_lintel_a_sill_and_jamb_stones_and_no_timber_lintel`, `a_cross_carved_in_a_wall_has_one_lintel_and_one_sill_across_its_width`; `sections`: `an_openings_head_is_sunk_behind_its_lintels_underside` | Closed |
| Any carved shape; a notch at a foot or a top | Framed along its own edges | `roles`: `a_hole_has_jambs_a_sill_and_a_head`; `layout`: `a_wall_being_carved_is_cut_and_framed_at_the_first_cut`; a notch probed clean | Closed |
| A door or shutters fitted | The prop's own frame, sized to its hole; the kit lays none there | `fitting`: `a_doors_frame_fills_its_hole_and_its_leaf_is_centred_in_the_frame`, `shutters_fill_their_opening_with_no_gap_at_any_edge` | Closed |
| A hole whose top is a roof cube | The roof's deck closes it, but no head is laid: the head belongs to the cube over it, which is the roof's | Probed clean | Gap 6 |

### Floors, decks, stairs and posts

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| A floor under a wall | Block: a border inside, a foot outside. Deck: boards run on to the logs' faces, or stop at another kind's | `roles`: `a_wall_on_a_floor_has_a_border_inside_and_a_foot_outside`; `floors`: `a_floor_inside_stone_footings_runs_on_under_its_log_walls_to_their_faces`; `sections`: `a_log_floor_in_a_stone_tower_stops_at_its_walls_faces` | Closed |
| A floor's open edge | Block: rounded, flagged; deck: a rim, joists where in the air | `floors`: `a_deck_out_in_the_open_has_rims_round_it_and_joists_under_it_in_the_air`, `a_floor_is_boarded_over_every_open_cube_and_nowhere_else` | Closed |
| A floor with a stairwell, or reaching past the wall under it | Block: soffit and rounding. Deck: rims round the hole and the overhang, butting at their corners; out over the air, open under its joists as a deck is | `sections`: `every_junction_case_is_drawn_without_a_fault` (a floor with a stairwell and a balcony) | Closed |
| A deck on posts; a post's top in a deck | Bearers on the posts, joists, rim (D306); the post's top cube is the deck's | `tests/decks.rs`; `inspect`: `a_posts_top_in_a_deck_is_as_solid_as_the_deck` | Closed |
| Stairs onto a deck, and from the ground | Stringers bear on the deck's rim and the ground | `tests/decks.rs`: `stairs_up_each_side_bear_on_the_rim_and_the_ground`; `stairs`: `a_flight_of_timber_is_laid_as_open_stairs_on_stringers_and_posts` | Closed |
| Stairs against a wall | The flight's own stringer or stone beside the wall's face; a stone stair's nose reaching back no further than its step and a hair into what rises behind it | `sections`: `every_junction_case_is_drawn_without_a_fault` (stairs against a wall); `sections`: `a_stone_stairs_noses_keep_out_of_the_flight_over_them` | Closed |
| A cut cube beside a whole one | A cut side shows over a lower cut; copings run on and mitre at a ramp's head and foot | `surface`: `a_side_shows_over_a_lower_cut_beside_it`; `sections`: `every_shape_is_drawn_in_every_kit_within_its_cut`, `a_ramps_coping_is_mitred_to_the_level_coping_at_its_head_and_its_foot` | Closed |

### Walls and roofs

Every row here is also a row of the roof–wall design's table, which is to become a row of the kit sheet and an inspected base in each wall kit under each roof.

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| A wall under a roof cube: a gable, the wedge under an eave | Carried up to the deck: a block surface cut at it, courses cut to the slope, eave blocking cut to the roof; no dress at the gable's steps | `sections`: `a_gable_is_cut_in_a_straight_line_to_the_roofs_deck_with_no_gap_under_it`; `roofs`: `a_stone_gable_rises_into_the_roof_cubes_over_it`; `runs`: `a_gable_end_is_logs_that_shorten_toward_the_ridge`, `a_wall_under_a_roof_is_closed_to_it_and_its_blocking_meets`; `roles`: `a_gable_carried_up_into_its_roof_has_no_edge_at_its_steps`; the checker's Under-the-roof rays | Closed |
| A roof's frame at a wall | Stops at the wall's face, never through a gable or out under a verge | `roofs`: `no_piece_of_a_frame_crosses_a_gable_or_ends_in_the_air_past_it`, `a_level_piece_of_frame_runs_only_over_the_rooms_of_any_plan` | Closed |
| Logs reaching a roof | Cut where the roof begins | `runs`: `no_log_passes_through_a_roof` | Closed |
| A roof on a cut wall cube or on posts | The roof stands on it; nothing to carry up | `sections`: `every_junction_case_is_drawn_without_a_fault` (a roof on posts, a roof on cut wall cubes) | Closed |
| A roof against a wall upslope (a lean-to's head) | Flashing; R3 adds the tuck | `roofs`: `a_roof_against_a_taller_wall_is_flashed_and_its_cubes_hold_by_their_ramp`, `every_open_side_of_a_roofs_covering_is_closed_by_one_edge_piece` (added with this page); probed clean | Closed; tuck Designed |
| A roof's verge against a wall | Flashing along the sloping edge; R4's `Step` | The same closing test; probed clean in logs and stone, hewn 1 coplanar pair | Designed |
| A roof falling to a wall (an eave or a hip's end against it) | Flashing exists, but the wall under the edge, carried up, comes through the covering (logs 4 to 20 cm) | Probed | Gap 2; Designed (R3, R4's `Gutter`) |
| A valley or inside corner at a wall; a hip or ridge at a wall | R4: butted in an inside corner, stopped at an outside one; a crease's cap stops at the face | No case | Designed |
| A wall or chimney up through a slope, a ridge or an eave (beside roof cubes) | Flashing on every side the roof meets it; but the log kind's end board against a chimney runs past the covering (5 to 27 cm), and the checker reads a chimney's dress above the roof as poking out | Probed | Gap 1; Designed (R3, R4) |
| A wall on a roof cube: on a slope, a corner, a ridge, a hip, a valley; a storey on the eaves | Refused today (D343) | `tests/rules.rs`: `nothing_but_a_roof_is_built_on_a_roof` | Closed by refusal; Designed (R1, R2, R6, R7) |

### Roofs with roofs

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| Ridge, hip, valley; a ridge a cube wide | Creases between squares, capped once | `roofs`: `a_gable_of_cubes_has_one_ridge_two_eaves_and_four_verges`, `a_hip_of_cubes_has_four_hips_and_a_short_ridge`, `an_l_of_cubes_has_a_valley_where_its_wings_meet_and_eaves_that_meet_in_it`, `an_odd_span_is_capped_by_one_ridge_down_its_flat_middle` | Closed |
| Eaves, verges and where they meet | Each open side exactly one edge piece; a hip's foot mitred; verges at an apex take no cap | `roofs`: `every_open_side_of_a_roofs_covering_is_closed_by_one_edge_piece`; `roofing`: `a_hips_capping_is_mitred_to_the_eaves_at_its_foot`, `an_eave_is_cut_plumb_through_the_coverings_thickness`; `sections`: `verges_meeting_at_an_apex_take_no_cap_there` | Closed |
| A wing's ridge into the main roof; a dormer | One roof: valleys where they meet, the wing's beam butting the main one | `roofs`: `a_wings_ridge_beam_butts_the_main_ones_side`; the checker's L, T, U plans and QA cabin's dormer | Closed |
| A roof over a roof, apart; an eave over a lower roof | Squares carry on only where their corners meet, so roofs at two heights stay apart | `roofs`: `a_storey_built_over_a_roof_stops_it_and_a_second_roof_over_it_is_apart`; `inspect`: `a_roofs_eave_over_a_lower_roof_is_measured_from_its_own_roof` | Closed |
| Two roof kinds meeting | As one roof; each square in its own kind | `sections`: `every_junction_case_is_drawn_without_a_fault` (two roof kinds meeting) | Closed |
| A roof kind cut to a stair, an upper half or a post | A flat top whose boards lie in one another (635 to 979 cm²) | Probed | Gap 4 |
| A hole carved in a roof | Near, closed; but eaves are carried into the hole, and far off the hull shows sky beside it | Probed | Gap 5 |

### Ground, change and loss

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| A wall's foot on the ground or a slope | A plinth stepping a course at a time and run on down into it; footings to the cube the ground passes through, set into it | `roles`: `a_foot_on_a_slope_steps_with_the_ground`; `tests/decks.rs`: `on_falling_ground_posts_stand_on_footings_at_the_ground_and_the_flight_reaches_the_deck` | Closed |
| A doorway above the ground | Door steps are cubes of the footing kind (D261) | `tests/rules.rs`: `everything_a_walker_stands_on_is_a_cube` | Closed |
| Carving, a cube at a time | The kit lays itself again round the gap, cut and framed at the first cut | `layout`: `a_wall_being_carved_is_cut_and_framed_at_the_first_cut`; `surface`: `an_l_and_a_carved_wall_stay_closed_with_relief` | Closed |
| A change of material | Within a pattern a repaint, the junctions unchanged (D298); across patterns the `meets` rows above | `ops`: `a_stone_wall_is_changed_to_logs_old_out_new_in`; `sections`: `kits_read_again_with_one_taken_out_added_or_reordered_lay_and_draw_without_fault` | Closed |
| Damage | A stone hole has sides and a back (D305); timber and roofs break in the shader | `sections`: `a_stone_wall_is_holed_where_its_stones_have_fallen_and_flat_where_not_harmed`; `sections`: `a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in` (damaged); the frames check's *damaged* (`cargo xtask artifacts --frames --checks damaged`): timber's and a roof's damage held still and shown | Closed |
| What is left after a collapse | Any arrangement is drawn (D192 rule 3); plans hanging from what fell go with it; a lintel left over air stops at its cube (`roles::End::Free`) | `loss`: `knocking_out_a_walls_base_course_brings_down_what_it_alone_held`, `plans_hanging_from_what_falls_go_with_it_and_no_others`; `sections`: `a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in` (after a collapse) | Closed |

### States, sections and detail

| Junction | What closes it | Proved by | Status |
|---|---|---|---|
| A section's edge | Pieces by anchor; a section is cut from the base laid whole | `sections`: `a_section_cut_from_its_base_laid_whole_is_the_section_laid_alone`, `every_piece_belongs_to_one_section_and_laying_a_section_lays_exactly_its_own`, `an_edit_laid_again_only_as_far_as_it_reaches_is_as_laid_whole` | Closed |
| Near, mid, far, hull | The same closed shape, coarser | The checker at all four details; `roofing`: `far_off_a_roofs_sheet_closes_over_its_hips_and_ridge` | Closed (Gap 5 aside) |
| A plan; a building going up | What is in laid from what is built (D358): a top, an end and a corner where it stops, a cube going in a top at its fill, all laid away as it is built on; ghosts drawn from the plan; what is in hides what is not; a wall carried up into a roof's cube in with the roof's first stage, an upright in with its first cube | `sections`: `a_plan_is_drawn_as_ghosts_a_piece_each_and_the_next_lit`, `a_plan_so_far_through_lights_one_piece_not_one_a_kit`; `sections`: `a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in` (as a plan, walls going up, roof going on, with no line through the top of what is in skipped), `a_stone_wall_built_halfway_along_has_an_end_and_a_top_where_it_stops`, `a_middle_cube_built_alone_has_its_edges_and_finishing_its_neighbour_takes_them_away`, `a_log_wall_going_up_ends_its_courses_where_what_is_in_stops`, `a_house_going_up_laid_again_a_fill_at_a_time_is_as_laid_whole`; the frames check's *going up* | Closed |

## 5. Patterns and materials

A junction rule is code, written once per pattern: the block surface and its roles, the coursed runs and their corners, the frame's members, the roof's squares and lines. A material is a file of a few lines over its pattern and its pack (D290), and it can change only:

- numbers the pattern reads: a bevel, a relief, a course, a member's section, an eave's reach;
- textures, and pieces by role from the pattern's shared set or its own pack, each of a stated size from its origin (D303), within an eighth of a cube where the role is a trim;
- how it meets other kinds: a `weight` and two trims chosen from a closed set (`Joint`: none, an end board, a cap), per kind if it must.

It cannot add a junction: its file's fields are closed, no game or layout code may name a material or branch on a kind's number (the test in `kit.rs`), and a piece cannot carry an offset. So a new material of a pattern inherits every row of the matrix that the pattern closes. What it can still break is a number or a piece, and those are checked: `cargo xtask kits` (sizes, the eighth of a cube, budgets, every role its pattern needs) and the ratchet, which inspects every material the settlement builds in. A material not in `assets/data/settlement.ron` must be inspected by name (`cargo xtask artifacts <name>`). A new pattern is code, and its own rows in this matrix.

## 6. How it is checked

| Check | What it runs | Invariants |
|---|---|---|
| Unit tests (`cargo xtask check`) | Each module's: roles, runs, roofs, roofing, surface, floors, stairs, trims, sections, fitting | 1, 2, 3, 5, each row it names |
| Rules tests (`tests/rules.rs`, `cargo xtask rules`) | Support, refusals, roofs on roofs, walking | 9 |
| Random operations (`ops`: `random_operations_keep_every_rule`, 64 seeds in the audit) | Every operation: nothing floats, laid twice the same, undo exact | 1, 7, 9 |
| The mesh checker (`cargo xtask artifacts`, D287) | Every baseline material, alone and in pairs, under every roof over every plan, the QA cabin, the castle and the junctions family (`junctions`), and the rectangle under a gable as a plan, going up, roofed by stages, damaged and after a collapse (counting see-through only within what is in), at four details: coplanar, doubled, see-through, under the roof, pokes out, buried, black, inward, not finite. Ratcheted on every commit (rectangles) and whole with `--ratchet` | 2, 3, 4, 6 |
| The frames checks (`cargo xtask artifacts --frames`) | Still views, the sun moving, a glide, a house going up, damage drawn only where there is damage, and timber's and roofs' damage holding still and showing | 6, 7 on the GPU |
| The kit checker and the model contract (`cargo xtask kits`, `cargo xtask contract`) | Every material's pieces, sizes, budgets and roles | 4, 8 |
| QA clips (`cargo xtask qa clips`) | Walls, openings, a roof by stages, a material swapped, a window carved, a storey, a deck, destruction, filmed per set | 7, by eye |
| The kit sheet (`--kit-sheet`) | Every case in every kit, built, going up and as a plan | 3, 7, by eye |

## Gaps

Found for this page by reading the code and probing each junction with the mesh checker. Each has the rule recommended to close it; none is built here.

1. **A wall or chimney through a roof, and a wall on one.** The roof is flashed where it meets a wall beside it, but a timber wall's end board against a chimney runs on past the covering, the checker reads a chimney's own dress above the roof as poking out, and no test or inspected base has the case. A wall on a roof cube is refused (D343). *Rule:* the roof–wall design, R1 to R7; its new rays into the wedge under a wall on a roof and down past a wall's face, and its kit-sheet rows and inspected bases, are this junction's proof. D343 is amended by the developers before R7 is built.
2. **A roof falling to a wall**, an eave or a hip's end against it. The wall under the edge, carried up into the roof cube, comes through the covering by 4 to 20 cm in timber, and a hip's piece stands 8 cm past the edge. *Rule:* R3 and R4's `Gutter`, and the wall under an edge against something cut to the deck as under an eave (blocked to the roof), never left to stand to its own top.
3. **Closed.** **Two cubes of a kind meeting only along an edge.** Each sees the other across the edge, so neither dresses it, while both dress the faces running up to it, and the two dresses overlap there. *Rule:* a shared edge is two outside edges, each cube dressing its own, its piece cut at the plane through the edge halfway between the two cubes, as a mitre.
4. **A roof kind cut to a stair, an upper half or a post** draws a flat top whose boards lie in one another. *Rule:* a roof kind takes only the shapes a roof has (whole, half, slopes and corner slopes), and the hammer offers no others in a roof material, read from the pattern's file, not code.
5. **A hole carved in a roof.** Eaves are carried into the hole, and far off the hull shows sky beside it. *Rule:* an opening in a roof is something standing beside it to its neighbours, so no eaves run into it and its sides take the curb and flashing of R4; the hull leaves it open at every detail.
6. **A hole whose top is a roof cube** has no head: the head belongs to the cube over the hole, and that is the roof's. *Rule:* a roof cube over a hole is the hole's head to the wall's kit, which lays its lintel, plate or head board at the cube's top.
7. **Closed.** **Timber floors and thick timber walls round a hole.** A deck's boards lie in the plane of the log ends under a stairwell or past the wall below (1,100 cm²), its overhang's corners show sky, and a thick log wall's ends lie in the plane of a window's hewn reveal. *Rule:* a deck's boards and rims stop a hair short of a sawn end's plane, as courses do, and a deck past the wall under it closes its corners with its rims, as at a free edge.
8. **Closed.** **States the mesh checker never sees.** It inspects finished bases only: no plan, no building part-built, no damaged base and nothing left after a collapse; timber's and roofs' damage are drawn only in the shader. *Rule:* inspect each case also as a plan, at each stage of fill and after the QA destroy clip's knock-outs, counting see-through only within what is in; and a frames check of damaged timber and roofs.
9. **Closed.** **Junctions closed but unproved**: a roof on posts or on cut wall cubes, two roof kinds meeting, stairs against a wall. Each probed clean, none a test or an inspected base. *Rule:* a *junctions* family in `inspect::cases`, a base for every row of this matrix in every baseline material, so the ratchet holds each one.
