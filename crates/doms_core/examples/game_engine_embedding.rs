//! Game Engine Embedding Demo with DOMS.js (Bevy / Godot / SDL Style).
//!
//! Demonstrates:
//! 1. Embedding DOMS within a native game loop.
//! 2. Exposing native Rust game entity state to JavaScript.
//! 3. Running game scripts for player controllers and AI behaviors.
//! 4. Frame-by-frame ticking with `update(deltaTime)`.

use doms_core::Engine;

#[derive(Debug, Clone)]
struct GameEntity {
    id: u32,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    health: i32,
}

fn main() {
    println!("============================================================");
    println!(" DOMS.js — Game Engine Embedding Demo (ECS / Game Loop)     ");
    println!("============================================================");

    let engine = Engine::new();
    let mut isolate = engine.create_isolate();
    let mut ctx = isolate.create_context();

    // Setup native entity in Rust
    let mut player = GameEntity {
        id: 1,
        x: 100.0,
        y: 200.0,
        vx: 0.0,
        vy: 0.0,
        health: 100,
    };

    println!("[Game Engine] Initial Player State: {:?}", player);

    // Inject Game API into JavaScript
    let init_script = format!(
        r#"
        var Player = {{
            id: {},
            x: {},
            y: {},
            vx: {},
            vy: {},
            health: {},
            takeDamage: function(amount) {{
                this.health -= amount;
                return this.health;
            }}
        }};

        // Game update script executed every frame
        function onUpdate(dt) {{
            // AI / Input Logic: move player towards target
            Player.vx = 50.0; // 50 px/sec
            Player.vy = -20.0;
            Player.x += Player.vx * dt;
            Player.y += Player.vy * dt;
            if (Player.x > 150.0) {{
                Player.takeDamage(10);
            }}
            return "Player at (" + Math.round(Player.x) + ", " + Math.round(Player.y) + ") HP: " + Player.health;
        }}
        "#,
        player.id, player.x, player.y, player.vx, player.vy, player.health
    );

    ctx.eval(&init_script).expect("Initialize game script");

    // ── Simulate 5 Game Loop Ticks ──
    println!("\n[Game Engine] Starting Game Loop Simulation (60 FPS, dt = 0.016s)...");
    let dt = 0.016;

    for frame in 1..=5 {
        let tick_call = format!("onUpdate({});", dt);
        let status = ctx.eval(&tick_call).expect("Execute onUpdate");

        // Sync back updated positions from JS to Rust
        let x_val = ctx.eval("Player.x").unwrap().to_number().unwrap_or(0.0);
        let y_val = ctx.eval("Player.y").unwrap().to_number().unwrap_or(0.0);
        let hp_val = ctx.eval("Player.health").unwrap().to_int32().unwrap_or(100);

        player.x = x_val;
        player.y = y_val;
        player.health = hp_val;

        println!("  [Frame {:02}] Script Status: {:?} | Rust Synced: (x: {:.1}, y: {:.1}, hp: {})",
            frame, status, player.x, player.y, player.health);
    }

    println!("\n[Game Engine] Final Native Player State: {:?}", player);
    println!("============================================================");
    println!(" Game Engine Embedding Demo Completed Successfully!         ");
    println!("============================================================");
}
