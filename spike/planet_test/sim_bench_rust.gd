extends SceneTree

# Runs the Rust extension's simulation tick from inside the engine. See sim_bench.gd
# for the GDScript version of the same workload.


func _init() -> void:
	if not ClassDB.class_exists("SimBench"):
		print("SIM rust extension did not load")
		quit(1)
		return
	for cells in [2562, 10242, 40962]:
		var bench = ClassDB.instantiate("SimBench")
		bench.setup(cells)
		var ms: float = bench.run(2000)
		var start := Time.get_ticks_usec()
		var field: PackedFloat32Array = bench.field(0)
		var copy_ms := (Time.get_ticks_usec() - start) / 1000.0
		print("SIM rust-in-godot cells=%d tick=%.4f ms, handing one field to the engine=%.4f ms (%d values)" % [
			cells, ms, copy_ms, field.size()])
	quit()
