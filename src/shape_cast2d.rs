use godot::prelude::*;
use godot::engine::Node2D;
use godot::engine::INode2D;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct YCShapeCast2D {

    base: Base<Node2D>,
}

#[godot_api]
impl INode2D for YCShapeCast2D {
    fn init(base: Base<Node2D>) -> Self {
        Self { base }
    }

    
}