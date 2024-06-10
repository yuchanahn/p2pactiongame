use godot::builtin::Vector2;

#[derive(Clone, Copy, Debug)]
pub enum Collision2D {
    Box(Box2D),
    Circle(Circle2D),
}

#[derive(Clone, Copy, Debug)]
pub struct Box2D {
    pub owner_id: u64,
    pub pos: Vector2,
    pub size: Vector2,
}

#[derive(Clone, Copy, Debug)]
pub struct Circle2D {
    pub owner_id: u64,
    pub pos: Vector2,
    pub radius: f32,
}

fn is_colliding_box_box(a: &Box2D, b: &Box2D) -> bool {
    a.pos.x < b.pos.x + b.size.x &&
    a.pos.x + a.size.x > b.pos.x &&
    a.pos.y < b.pos.y + b.size.y &&
    a.pos.y + a.size.y > b.pos.y
}

fn is_colliding_box_circle(a: &Box2D, b: &Circle2D) -> bool {
    let closest_x = (b.pos.x).max(a.pos.x).min(a.pos.x + a.size.x);
    let closest_y = (b.pos.y).max(a.pos.y).min(a.pos.y + a.size.y);

    let distance_x = b.pos.x - closest_x;
    let distance_y = b.pos.y - closest_y;

    (distance_x * distance_x + distance_y * distance_y) < (b.radius * b.radius)
}

fn is_colliding_circle_circle(a: &Circle2D, b: &Circle2D) -> bool {
    let distance = (a.pos.x - b.pos.x) * (a.pos.x - b.pos.x) + (a.pos.y - b.pos.y) * (a.pos.y - b.pos.y);
    let radius_sum = a.radius + b.radius;
    distance < radius_sum * radius_sum
}

pub fn box_cast(world: &Vec<Collision2D>, box2d: &Box2D) -> Vec<Collision2D> {
    let r = world.iter().filter(|collision| match collision {
        Collision2D::Box(b) => is_colliding_box_box(box2d, b),
        Collision2D::Circle(c) => is_colliding_box_circle(box2d, c),
    }).map(|x| *x).collect();

    r
}

pub fn circle_cast(world: &Vec<Collision2D>, circle2d: &Circle2D) -> Vec<Collision2D> {
    let r = world.iter().filter(|collision| match collision {
        Collision2D::Box(b) => is_colliding_box_circle(b, circle2d),
        Collision2D::Circle(c) => is_colliding_circle_circle(circle2d, c),
    }).map(|x| *x).collect();

    r
}