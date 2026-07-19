extends Control

# Golden test 导出脚本
# 场景加载后，等一帧让布局完成，然后导出所有 Control 的最终 position+size

func _ready():
    # 等一帧，确保布局完成
    await get_tree().process_frame
    await get_tree().process_frame  # Godot 布局可能需要两帧才稳定

    var result = {}
    _collect_layout(self, "", result)

    # 导出为 JSON（写到用户目录）
    var json = JSON.stringify(result, "  ")
    var file = FileAccess.open("user://layout_golden.json", FileAccess.WRITE)
    file.store_string(json)
    file.close()

    print("=== Golden layout exported ===")
    print(json)
    print("=== Written to: ", DirAccess.get_user_dir(), "/layout_golden.json ===")

    # 退出（headless 模式下）
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
