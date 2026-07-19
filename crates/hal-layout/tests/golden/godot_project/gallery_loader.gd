extends SceneTree

func _init():
	var scene = load("res://control_gallery.tscn").instantiate()
	root.add_child(scene)

	# 强制 Root 尺寸为 960x640
	# 用 set_deferred 避免 _ready 里的 warning
	if scene is Control:
		scene.set_deferred("size", Vector2i(960, 640))

	await process_frame
	await process_frame
	await process_frame

	# 再次强制（覆盖 tree.gd 可能的修改）
	if scene is Control:
		scene.size = Vector2i(960, 640)
	await process_frame

	var result = {}
	_collect_layout(scene, "", result)

	var json = JSON.stringify(result, "  ")
	print("=== GOLDEN_START ===")
	print(json)
	print("=== GOLDEN_END ===")

	var file = FileAccess.open("user://gallery_golden.json", FileAccess.WRITE)
	file.store_string(json)
	file.close()

	print("=== Root size: ", scene.size, " ===")
	print("=== Nodes: ", result.size(), " ===")

	quit()

func _collect_layout(node: Node, path: String, result: Dictionary):
	if node is Control:
		var control = node as Control
		# 用 position + size（相对父节点），不用 global_rect
		# 这样能排除 Godot headless 的全局偏移
		result[path] = {
			"name": node.name,
			"x": control.position.x,
			"y": control.position.y,
			"width": control.size.x,
			"height": control.size.y,
		}
	for child in node.get_children():
		var child_path = path + "/" + child.name if path != "" else child.name
		_collect_layout(child, child_path, result)
