extends SceneTree

# Benchmark for D22: how long does one tick of a planet-sized cell simulation take in
# GDScript? Each cell has six neighbours and four fields; a tick diffuses every field
# and applies a little coupling between them, which is the shape of the real workload.

const FIELDS := 4
const NEIGHBOURS := 6


func _init() -> void:
	for cells in [2562, 10242, 40962]:
		var neighbours := PackedInt32Array()
		neighbours.resize(cells * NEIGHBOURS)
		var rng := RandomNumberGenerator.new()
		rng.seed = 7
		for i in neighbours.size():
			neighbours[i] = rng.randi_range(0, cells - 1)
		var current: Array[PackedFloat32Array] = []
		var next: Array[PackedFloat32Array] = []
		for f in FIELDS:
			var values := PackedFloat32Array()
			values.resize(cells)
			for i in cells:
				values[i] = rng.randf()
			current.append(values)
			next.append(values.duplicate())

		var ticks := 20
		var start := Time.get_ticks_usec()
		for t in ticks:
			_tick(cells, neighbours, current, next)
			var swap := current
			current = next
			next = swap
		var ms := (Time.get_ticks_usec() - start) / 1000.0 / ticks
		print("SIM gdscript cells=%d tick=%.2f ms checksum=%.4f" % [cells, ms, current[0][cells / 2]])
	quit()


func _tick(cells: int, neighbours: PackedInt32Array, current: Array[PackedFloat32Array],
		next: Array[PackedFloat32Array]) -> void:
	for f in FIELDS:
		var src := current[f]
		var dst := next[f]
		var other := current[(f + 1) % FIELDS]
		for i in cells:
			var base := i * NEIGHBOURS
			var sum := 0.0
			for n in NEIGHBOURS:
				sum += src[neighbours[base + n]]
			var v := src[i]
			v += 0.1 * (sum / NEIGHBOURS - v)
			v += 0.01 * (other[i] - v)
			dst[i] = clampf(v, 0.0, 1.0)
