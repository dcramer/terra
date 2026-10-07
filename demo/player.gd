extends CharacterBody3D

# First-person player on a round planet. "Down" is always toward the planet's centre.
# Walks with gravity, or flies freely to reach orbit.

const WALK_SPEED := 6.0
const RUN_SPEED := 14.0
const JUMP_SPEED := 6.5
const GRAVITY := 14.0
const FLY_SPEED := 40.0
const FLY_FAST_SPEED := 500.0
const EYE_HEIGHT := 1.6
const MOUSE_SENSITIVITY := 0.0025

var flying := false
var controls_enabled := true
# Voxel data is the authority on where the ground is, so the player can stand on terrain
# whose collision mesh has not been built yet.
var ground_tool: VoxelToolLodTerrain

# When set, the camera looks at this point instead of where the player is facing.
var look_target: Variant = null

var camera := Camera3D.new()
var _pitch := 0.0
var _forward := Vector3.FORWARD


func _ready() -> void:
	var shape := CollisionShape3D.new()
	var capsule := CapsuleShape3D.new()
	capsule.radius = 0.35
	capsule.height = 1.8
	shape.shape = capsule
	shape.position.y = 0.9
	add_child(shape)
	# The camera follows the body but is updated every rendered frame, so looking around
	# is as smooth as the display allows rather than tied to the 60 Hz physics step.
	camera.top_level = true
	camera.physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	camera.near = 0.1
	camera.far = 60000.0
	camera.fov = 80.0
	add_child(camera)
	camera.make_current()
	# The terrain loads around this, with collision.
	var viewer := VoxelViewer.new()
	viewer.view_distance = 30000
	camera.add_child(viewer)
	floor_max_angle = deg_to_rad(55.0)


# Puts the player at a point, facing along the ground toward `look_at_point`.
func place(point: Vector3, look_at_point: Vector3, pitch := 0.0) -> void:
	global_position = point
	velocity = Vector3.ZERO
	var up := point.normalized()
	var toward := (look_at_point - point).slide(up)
	if toward.length() > 0.001:
		_forward = toward.normalized()
	_pitch = pitch
	look_target = null
	_align(up)
	reset_physics_interpolation()
	_update_camera()


func look_direction() -> Vector3:
	return -camera.global_basis.z


func _unhandled_input(event: InputEvent) -> void:
	if not controls_enabled:
		return
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		_forward = _forward.rotated(global_position.normalized(), -event.relative.x * MOUSE_SENSITIVITY)
		_pitch = clampf(_pitch - event.relative.y * MOUSE_SENSITIVITY, -1.5, 1.5)
	elif event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_F:
		flying = not flying
		velocity = Vector3.ZERO


func _align(up: Vector3) -> void:
	_forward = _forward.slide(up).normalized()
	var right := _forward.cross(up)
	global_basis = Basis(right, up, -_forward)


func _process(_delta: float) -> void:
	_update_camera()


func _update_camera() -> void:
	var body := get_global_transform_interpolated() if is_inside_tree() else global_transform
	var up := body.origin.normalized()
	var forward := _forward.slide(up).normalized()
	var facing := Basis(forward.cross(up), up, -forward)
	camera.global_transform = Transform3D(
		facing * Basis(Vector3.RIGHT, _pitch), body.origin + up * EYE_HEIGHT)
	if look_target != null:
		camera.look_at(look_target, Vector3.UP)


func _physics_process(delta: float) -> void:
	var up := global_position.normalized()
	_align(up)
	up_direction = up
	var right := global_basis.x

	var forward_input := 0.0
	var side_input := 0.0
	var rise_input := 0.0
	if controls_enabled:
		forward_input = float(Input.is_key_pressed(KEY_W)) - float(Input.is_key_pressed(KEY_S))
		side_input = float(Input.is_key_pressed(KEY_D)) - float(Input.is_key_pressed(KEY_A))
		rise_input = float(Input.is_key_pressed(KEY_SPACE)) - float(Input.is_key_pressed(KEY_CTRL))
	var fast := Input.is_key_pressed(KEY_SHIFT)

	if flying:
		var direction := look_direction() * forward_input + right * side_input + up * rise_input
		velocity = direction.limit_length(1.0) * (FLY_FAST_SPEED if fast else FLY_SPEED)
		move_and_slide()
	else:
		var vertical := velocity.dot(up) - GRAVITY * delta
		var grounded := is_on_floor() or _depth_in_ground() > -0.05
		if grounded:
			vertical = maxf(vertical, 0.0)
			if rise_input > 0.0:
				vertical = JUMP_SPEED
		var walk := (_forward * forward_input + right * side_input).limit_length(1.0)
		velocity = walk * (RUN_SPEED if fast else WALK_SPEED) + up * vertical
		move_and_slide()

	# Never end a frame inside the ground, whatever the collision meshes say.
	var depth := _depth_in_ground()
	if depth > 0.0:
		global_position += up * depth


# How far the player's feet are below the ground surface. Negative when above it.
func _depth_in_ground() -> float:
	if ground_tool == null:
		return -1000.0
	return -ground_tool.get_voxel_f_interpolated(global_position)
