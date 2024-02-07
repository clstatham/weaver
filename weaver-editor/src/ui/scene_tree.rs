use weaver::prelude::*;

pub fn scene_tree_ui(world: &World, ui: &mut egui::Ui) {
    // for node in graph.orphans() {
    //     let name = node.to_string();

    //     scene_tree_ui_recurse(world, ui, &node, &name);
    // }
}

// fn scene_tree_ui_recurse(world: &World, ui: &mut egui::Ui, node: &ValueUid, name: &str) {
//     let graph = world.graph();
//     let children: Vec<_> = graph
//         .get_child_edges(node)
//         .map(|i| i.collect())
//         .unwrap_or_default();

//     egui::CollapsingHeader::new(name).show(ui, |ui| {
//         for child in children {
//             let child_name = child
//                 .payload
//                 .as_deref()
//                 .map(|s| s.to_string())
//                 .unwrap_or(child.child.to_string());
//             scene_tree_ui_recurse(world, ui, &child.child, &child_name);
//         }
//     });
// }
