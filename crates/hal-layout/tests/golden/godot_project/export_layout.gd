extends Control

# Golden test 导出脚本
# 场景加载后，等帧让布局完成，然后导出所有 Control 的最终 position+size

func _ready():
	await get_tree().process_frame
	await get_tree().process_frame

	var result = {}
	_collect_layout(self, "", result)

	var json = JSON.stringify(result, "  ")
	print("=== GOLDEN_START ===")
	print(json)
	print("=== GOLDEN_END ===")

	var file = FileAccess.open("user://layout_golden.json", FileAccess.WRITE)
	file.store_string(json)
	file.close()

	print("=== Written to: ", OS.get_user_data_dir(), "/layout_golden.json ===")

	get_tree().quit()

func _collect_layout(node: Node, path: String, result: Dictionary):
	if node is Control:
		var control = node as Control
		var rect = control.get_global_rect()
		result[path] = {
			"name": node.name,
			"x": rect.position.x,
			"y": rect.position.y,
			"width": rect.size.x,
			"height": rect.size.y,
		}
	for child in node.get_children():
		var child_path = path + "/" + child.name if path != "" else child.name
		_collect_layout(child, child_path, result)
