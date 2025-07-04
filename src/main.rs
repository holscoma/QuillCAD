use bevy::{prelude::*, window::PrimaryWindow};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

/// アプリケーション全体の状態
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
enum AppState {
    #[default]
    Viewing,
    Sketching,
}

/// 現在選択されているスケッチツール
#[derive(Resource, Debug, Clone, Eq, PartialEq, Default)]
enum ActiveSketchTool {
    #[default]
    Line,
    Circle,
    Rectangle,
}

/// スケッチモードで非表示にするメインの立方体
#[derive(Component)]
struct MainCube;

/// スケッチの基準となる平面
#[derive(Component)]
struct SketchPlane;

/// スケッチデータを保持するリソース
#[derive(Resource, Default)]
struct SketchData {
    lines: Vec<[Vec3; 2]>,
    circles: Vec<(Vec3, f32)>,
    rectangles: Vec<[Vec3; 2]>,
    start_point: Option<Vec3>,
}

fn main() {
    App::new()
        .init_state::<AppState>()
        .init_resource::<SketchData>()
        .init_resource::<ActiveSketchTool>()
        .add_plugins(DefaultPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(EguiPlugin)
        .add_systems(Startup, (setup, configure_fonts))
        .add_systems(Update, ui_system)
        .add_systems(OnEnter(AppState::Sketching), on_sketch_enter)
        .add_systems(
            Update,
            (
                sketching_system,
                draw_sketch_gizmos,
                draw_grid,
            )
            .run_if(in_state(AppState::Sketching)),
        )
        .add_systems(OnExit(AppState::Sketching), on_sketch_exit)
        .run();
}

/// UIを描画するシステム
fn ui_system(
    mut contexts: EguiContexts,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut active_tool: ResMut<ActiveSketchTool>,
) {
    egui::SidePanel::left("side_panel").show(contexts.ctx_mut(), |ui| {
        ui.heading("QuillCAD");
        ui.separator();

        match current_state.get() {
            AppState::Viewing => {
                if ui.button("スケッチ開始").clicked() {
                    next_state.set(AppState::Sketching);
                }
            }
            AppState::Sketching => {
                ui.label("スケッチモード");
                ui.separator();

                ui.label("ツール選択");
                let _ = ui.selectable_value(active_tool.as_mut(), ActiveSketchTool::Line, "直線");
                let _ = ui.selectable_value(active_tool.as_mut(), ActiveSketchTool::Circle, "円");
                let _ = ui.selectable_value(active_tool.as_mut(), ActiveSketchTool::Rectangle, "四角形");

                ui.separator();

                if ui.button("スケッチ完了").clicked() {
                    next_state.set(AppState::Viewing);
                }
            }
        }
    });
}

/// Sketching状態に入った時に呼ばれる関数
fn on_sketch_enter(
    mut sketch_data: ResMut<SketchData>,
    mut cube_query: Query<&mut Visibility, (With<MainCube>, Without<SketchPlane>)>,
    mut plane_query: Query<&mut Visibility, (With<SketchPlane>, Without<MainCube>)>,
    mut camera_query: Query<(&mut Transform, &mut PanOrbitCamera), With<Camera3d>>,
) {
    println!("スケッチモードに入りました.");
    *sketch_data = SketchData::default();

    let mut cube_visibility = cube_query.single_mut();
    *cube_visibility = Visibility::Hidden;

    let mut plane_visibility = plane_query.single_mut();
    *plane_visibility = Visibility::Visible;

    let (mut transform, mut pan_orbit) = camera_query.single_mut();
    *transform = Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z);
    pan_orbit.button_orbit = MouseButton::Middle;
}

/// Sketching状態から出る時に呼ばれる関数
fn on_sketch_exit(
    mut sketch_data: ResMut<SketchData>,
    mut cube_query: Query<&mut Visibility, (With<MainCube>, Without<SketchPlane>)>,
    mut plane_query: Query<&mut Visibility, (With<SketchPlane>, Without<MainCube>)>,
    mut camera_query: Query<&mut PanOrbitCamera>,
) {
    println!("表示モードに戻ります.");
    *sketch_data = SketchData::default();

    let mut cube_visibility = cube_query.single_mut();
    *cube_visibility = Visibility::Visible;

    let mut plane_visibility = plane_query.single_mut();
    *plane_visibility = Visibility::Hidden;

    let mut pan_orbit = camera_query.single_mut();
    pan_orbit.button_orbit = MouseButton::Left;
}

/// スクリーン座標からXZ平面上のワールド座標を計算する
fn screen_to_world(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec3> {
    window.cursor_position().and_then(|cursor_pos| {
        let plane_origin = Vec3::ZERO;
        let plane_normal = Vec3::Y;
        
        camera.viewport_to_world(camera_transform, cursor_pos).and_then(|ray| {
            ray.intersect_plane(plane_origin, Plane3d::new(plane_normal))
                .map(|distance| ray.get_point(distance))
        })
    })
}

/// スケッチの入力とロジックを処理するシステム
fn sketching_system(
    mut contexts: EguiContexts,
    mut sketch_data: ResMut<SketchData>,
    active_tool: Res<ActiveSketchTool>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
) {
    if contexts.ctx_mut().is_using_pointer() {
        return;
    }

    let window = q_window.single();
    let (camera, camera_transform) = q_camera.single();

    if let Some(world_pos) = screen_to_world(window, camera, camera_transform) {
        if mouse_buttons.just_pressed(MouseButton::Left) {
            if let Some(start_pos) = sketch_data.start_point {
                match *active_tool {
                    ActiveSketchTool::Line => {
                        sketch_data.lines.push([start_pos, world_pos]);
                    }
                    ActiveSketchTool::Circle => {
                        let radius = start_pos.distance(world_pos);
                        sketch_data.circles.push((start_pos, radius));
                    }
                    ActiveSketchTool::Rectangle => {
                        sketch_data.rectangles.push([start_pos, world_pos]);
                    }
                }
                sketch_data.start_point = None;
            } else {
                sketch_data.start_point = Some(world_pos);
            }
        }

        if mouse_buttons.just_pressed(MouseButton::Right) {
            sketch_data.start_point = None;
        }
    }
}

/// スケッチのジオメトリをGizmosで描画するシステム
fn draw_sketch_gizmos(
    mut gizmos: Gizmos,
    sketch_data: Res<SketchData>,
    active_tool: Res<ActiveSketchTool>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
) {
    // 完成した線を描画
    for line in &sketch_data.lines {
        gizmos.line(line[0], line[1], Color::WHITE);
    }
    // 完成した円を描画
    for circle in &sketch_data.circles {
        gizmos.circle(circle.0, Direction3d::Y, circle.1, Color::WHITE);
    }
    // 完成した四角形を描画
    for rect in &sketch_data.rectangles {
        draw_rectangle(&mut gizmos, rect[0], rect[1], Color::WHITE);
    }

    // 描画中のプレビューを描画
    if let Some(start_point) = sketch_data.start_point {
        let window = q_window.single();
        let (camera, camera_transform) = q_camera.single();
        if let Some(world_pos) = screen_to_world(window, camera, camera_transform) {
            match *active_tool {
                ActiveSketchTool::Line => {
                    gizmos.line(start_point, world_pos, Color::YELLOW);
                }
                ActiveSketchTool::Circle => {
                    let radius = start_point.distance(world_pos);
                    gizmos.circle(start_point, Direction3d::Y, radius, Color::YELLOW);
                }
                ActiveSketchTool::Rectangle => {
                    draw_rectangle(&mut gizmos, start_point, world_pos, Color::YELLOW);
                }
            }
        }
    }
}

/// 2つの対角点から四角形をGizmosで描画するヘルパー関数
fn draw_rectangle(gizmos: &mut Gizmos, p1: Vec3, p2: Vec3, color: Color) {
    let corner2 = Vec3::new(p1.x, 0.0, p2.z);
    let corner4 = Vec3::new(p2.x, 0.0, p1.z);
    gizmos.line(p1, corner2, color);
    gizmos.line(corner2, p2, color);
    gizmos.line(p2, corner4, color);
    gizmos.line(corner4, p1, color);
}

/// スケッチ平面にグリッドを描画するシステム
fn draw_grid(mut gizmos: Gizmos) {
    let size = 10.0;
    let step = 1.0;
    let num_lines = (size / step) as i32;

    for i in -num_lines..=num_lines {
        let i_f32 = i as f32 * step;
        gizmos.line(Vec3::new(-size, 0.0, i_f32), Vec3::new(size, 0.0, i_f32), Color::GRAY);
        gizmos.line(Vec3::new(i_f32, 0.0, -size), Vec3::new(i_f32, 0.0, size), Color::GRAY);
    }
    gizmos.line(Vec3::new(-size, 0.0, 0.0), Vec3::new(size, 0.0, 0.0), Color::RED);
    gizmos.line(Vec3::new(0.0, 0.0, -size), Vec3::new(0.0, 0.0, size), Color::BLUE);
}

/// eguiのフォントを設定するシステム
fn configure_fonts(mut contexts: EguiContexts) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSansJP-Regular.ttf")),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "my_font".to_owned());
    fonts.families.entry(egui::FontFamily::Monospace).or_default().push("my_font".to_owned());
    contexts.ctx_mut().set_fonts(fonts);
}

/// 3Dシーンをセットアップするためのシステム
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(10.0, 10.0)),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3)),
            visibility: Visibility::Hidden,
            ..default()
        },
        SketchPlane,
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(Color::rgb(0.8, 0.7, 0.6)),
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..default()
        },
        MainCube,
    ));

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        PanOrbitCamera::default(),
    ));
}
