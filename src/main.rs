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
    Select,
}

// ActiveSketchToolリソースの値に基づいてシステムを実行するためのカスタム条件
fn is_active_tool(tool: ActiveSketchTool) -> impl Fn(Res<ActiveSketchTool>) -> bool + Clone {
    move |active_tool: Res<ActiveSketchTool>| *active_tool == tool
}

/// スケッチモードで非表示にするメインの立方体
#[derive(Component)]
struct MainCube;

/// スケッチの基準となる平面
#[derive(Component)]
struct SketchPlane;

/// 直線スケッチのコンポーネント
#[derive(Component, Debug)]
struct SketchLine {
    p1: Vec3,
    p2: Vec3,
}

/// 円スケッチのコンポーネント
#[derive(Component, Debug)]
struct SketchCircle {
    center: Vec3,
    radius: f32,
}

/// 四角形スケッチのコンポーネント
#[derive(Component, Debug)]
struct SketchRectangle {
    p1: Vec3,
    p2: Vec3,
}

/// スケッチが選択されていることを示すマーカーコンポーネント
#[derive(Component, Default)]
struct Selected;

/// スケッチデータを保持するリソース
#[derive(Resource, Default)]
struct SketchData {
    start_point: Option<Vec3>,
    extrude_distance: f32,
}

/// 押し出し処理をトリガーするイベント
#[derive(Event)]
struct ExtrudeEvent;

fn main() {
    App::new()
        .init_state::<AppState>()
        .init_resource::<SketchData>()
        .init_resource::<ActiveSketchTool>()
        .add_event::<ExtrudeEvent>() // ExtrudeEventを登録
        .add_plugins(DefaultPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(EguiPlugin)
        .add_systems(Startup, (setup, configure_fonts))
        .add_systems(Update, ui_system)
        .add_systems(OnEnter(AppState::Sketching), on_sketch_enter)
        .add_systems(
            Update,
            (
                sketching_system.run_if(is_active_tool(ActiveSketchTool::Line)),
                sketching_system.run_if(is_active_tool(ActiveSketchTool::Circle)),
                sketching_system.run_if(is_active_tool(ActiveSketchTool::Rectangle)),
                selection_system.run_if(is_active_tool(ActiveSketchTool::Select)),
                draw_sketch_gizmos,
                draw_grid,
                extrude_system, // 押し出しシステムを追加
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
    mut sketch_data: ResMut<SketchData>,
    mut extrude_events: EventWriter<ExtrudeEvent>,
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
                let _ = ui.selectable_value(active_tool.as_mut(), ActiveSketchTool::Select, "選択");

                ui.separator();

                ui.label("押し出し");
                ui.add(egui::DragValue::new(&mut sketch_data.extrude_distance).speed(0.1).suffix("m"));
                if ui.button("押し出し").clicked() {
                    extrude_events.send(ExtrudeEvent);
                }

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
    mut commands: Commands,
    mut sketch_data: ResMut<SketchData>,
    mut cube_query: Query<&mut Visibility, (With<MainCube>, Without<SketchPlane>)>,
    mut plane_query: Query<&mut Visibility, (With<SketchPlane>, Without<MainCube>)>,
    mut camera_query: Query<(&mut Transform, &mut PanOrbitCamera), With<Camera3d>>,
    // 既存のスケッチエンティティを削除するためのクエリ
    q_lines: Query<Entity, With<SketchLine>>,
    q_circles: Query<Entity, With<SketchCircle>>,
    q_rectangles: Query<Entity, With<SketchRectangle>>,
) {
    println!("スケッチモードに入りました.");
    *sketch_data = SketchData::default();

    // 既存のスケッチエンティティをすべて削除
    for entity in q_lines.iter() {
        commands.entity(entity).despawn();
    }
    for entity in q_circles.iter() {
        commands.entity(entity).despawn();
    }
    for entity in q_rectangles.iter() {
        commands.entity(entity).despawn();
    }

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
    *sketch_data = SketchData::default(); // start_pointをクリア

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
    mut commands: Commands,
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
                        commands.spawn(SketchLine { p1: start_pos, p2: world_pos });
                    }
                    ActiveSketchTool::Circle => {
                        let radius = start_pos.distance(world_pos);
                        commands.spawn(SketchCircle { center: start_pos, radius });
                    }
                    ActiveSketchTool::Rectangle => {
                        commands.spawn(SketchRectangle { p1: start_pos, p2: world_pos });
                    }
                    _ => {},
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

/// スケッチの選択を処理するシステム
fn selection_system(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    q_lines: Query<(Entity, &SketchLine, Option<&Selected>)>,
    q_circles: Query<(Entity, &SketchCircle, Option<&Selected>)>,
    q_rectangles: Query<(Entity, &SketchRectangle, Option<&Selected>)>,
) {
    if contexts.ctx_mut().is_using_pointer() {
        return;
    }

    let window = q_window.single();
    let (camera, camera_transform) = q_camera.single();

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(world_mouse_pos) = screen_to_world(window, camera, camera_transform) {
            let mut closest_entity: Option<Entity> = None;
            let mut min_distance_sq = f32::MAX;
            let tolerance_sq = 0.1 * 0.1; // 選択の許容範囲の二乗

            // 直線の選択判定
            for (entity, line, _) in q_lines.iter() {
                let dist_sq = point_line_segment_distance_sq(world_mouse_pos, line.p1, line.p2);
                if dist_sq < tolerance_sq && dist_sq < min_distance_sq {
                    min_distance_sq = dist_sq;
                    closest_entity = Some(entity);
                }
            }

            // 円の選択判定
            for (entity, circle, _) in q_circles.iter() {
                let dist_sq = world_mouse_pos.distance_squared(circle.center);
                // 円周からの距離を考慮
                let dist_from_circumference_sq = (dist_sq.sqrt() - circle.radius).powi(2);
                if dist_from_circumference_sq < tolerance_sq && dist_from_circumference_sq < min_distance_sq {
                    min_distance_sq = dist_from_circumference_sq;
                    closest_entity = Some(entity);
                }
            }

            // 四角形の選択判定
            for (entity, rect, _) in q_rectangles.iter() {
                // 四角形の境界ボックス内にあるか、または境界線に近いか
                let min_x = rect.p1.x.min(rect.p2.x);
                let max_x = rect.p1.x.max(rect.p2.x);
                let min_z = rect.p1.z.min(rect.p2.z);
                let max_z = rect.p1.z.max(rect.p2.z);

                let is_inside_x = world_mouse_pos.x >= min_x && world_mouse_pos.x <= max_x;
                let is_inside_z = world_mouse_pos.z >= min_z && world_mouse_pos.z <= max_z;

                // 簡易的な境界線判定（より正確には各線分との距離を測るべきだが、今回は簡易化）
                let is_near_border = (world_mouse_pos.x - min_x).abs() < tolerance_sq.sqrt() ||
                                     (world_mouse_pos.x - max_x).abs() < tolerance_sq.sqrt() ||
                                     (world_mouse_pos.z - min_z).abs() < tolerance_sq.sqrt() ||
                                     (world_mouse_pos.z - max_z).abs() < tolerance_sq.sqrt();

                if (is_inside_x && is_inside_z) || is_near_border {
                    // 四角形の場合、距離計算が複雑なので、一旦ヒットしたものを選択対象とする
                    // より正確な距離計算が必要であれば、各辺との距離を計算する
                    if 0.0 < min_distance_sq { // 既に他の図形がヒットしている場合は、そちらを優先しない
                        min_distance_sq = 0.0; // ヒットしたとみなす
                        closest_entity = Some(entity);
                    }
                }
            }

            // 既存の選択をすべて解除
            for (entity, _, selected) in q_lines.iter() {
                if selected.is_some() {
                    commands.entity(entity).remove::<Selected>();
                }
            }
            for (entity, _, selected) in q_circles.iter() {
                if selected.is_some() {
                    commands.entity(entity).remove::<Selected>();
                }
            }
            for (entity, _, selected) in q_rectangles.iter() {
                if selected.is_some() {
                    commands.entity(entity).remove::<Selected>();
                }
            }

            // 新しい選択を適用
            if let Some(entity) = closest_entity {
                commands.entity(entity).insert(Selected);
            }
        }
    }
}

/// 押し出し処理を行うシステム
fn extrude_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sketch_data: Res<SketchData>,
    mut extrude_events: EventReader<ExtrudeEvent>,
    q_selected_lines: Query<(Entity, &SketchLine), With<Selected>>,
    q_selected_circles: Query<(Entity, &SketchCircle), With<Selected>>,
    q_selected_rectangles: Query<(Entity, &SketchRectangle), With<Selected>>,
) {
    use bevy::render::render_asset::RenderAssetUsages;

    for _event in extrude_events.read() {
        println!("押し出しイベントを受信しました。");

        let extrude_distance = sketch_data.extrude_distance;

        // 選択された直線からの押し出し（面を生成）
        for (entity, line) in q_selected_lines.iter() {
            println!("直線から押し出し: {:?}", line);
            // 直線を押し出すと面になる。頂点とインデックスを直接定義
            let p1 = line.p1;
            let p2 = line.p2;
            let normal = Vec3::Y; // XZ平面からの押し出し

            let v1 = p1;
            let v2 = p2;
            let v3 = p2 + normal * extrude_distance;
            let v4 = p1 + normal * extrude_distance;

            let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![v1.to_array(), v2.to_array(), v3.to_array(), v4.to_array()]);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]]);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            mesh.insert_indices(bevy::render::mesh::Indices::U32(vec![0, 1, 2, 0, 2, 3]));

            commands.spawn(PbrBundle {
                mesh: meshes.add(mesh),
                material: materials.add(Color::rgb(0.7, 0.7, 0.7)),
                ..default()
            });
            commands.entity(entity).insert(Visibility::Hidden); // 元のスケッチを非表示
        }

        // 選択された円からの押し出し（円柱を生成）
        for (entity, circle) in q_selected_circles.iter() {
            println!("円から押し出し: {:?}", circle);
            commands.spawn(PbrBundle {
                mesh: meshes.add(Cylinder::new(circle.radius, extrude_distance).mesh()),
                material: materials.add(Color::rgb(0.7, 0.7, 0.7)),
                transform: Transform::from_xyz(circle.center.x, extrude_distance / 2.0, circle.center.z),
                ..default()
            });
            commands.entity(entity).insert(Visibility::Hidden); // 元のスケッチを非表示
        }

        // 選択された四角形からの押し出し（直方体を生成）
        for (entity, rect) in q_selected_rectangles.iter() {
            println!("四角形から押し出し: {:?}", rect);
            let min_x = rect.p1.x.min(rect.p2.x);
            let max_x = rect.p1.x.max(rect.p2.x);
            let min_z = rect.p1.z.min(rect.p2.z);
            let max_z = rect.p1.z.max(rect.p2.z);

            let width = max_x - min_x;
            let depth = max_z - min_z;
            let center_x = (min_x + max_x) / 2.0;
            let center_z = (min_z + max_z) / 2.0;

            commands.spawn(PbrBundle {
                mesh: meshes.add(Cuboid::new(width, extrude_distance, depth).mesh()),
                material: materials.add(Color::rgb(0.7, 0.7, 0.7)),
                transform: Transform::from_xyz(center_x, extrude_distance / 2.0, center_z),
                ..default()
            });
            commands.entity(entity).insert(Visibility::Hidden); // 元のスケッチを非表示
        }
    }
}

/// 点と線分の最短距離の二乗を計算するヘルパー関数
fn point_line_segment_distance_sq(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ap = p - a;
    let ab = b - a;
    let ab_len_sq = ab.length_squared();
    if ab_len_sq == 0.0 { // 線分が点の場合
        return ap.length_squared();
    }
    let t = ap.dot(ab) / ab_len_sq;
    if t < 0.0 { // 線分のA点の外側
        return ap.length_squared();
    } else if t > 1.0 { // 線分のB点の外側
        return (p - b).length_squared();
    } else { // 線分上
        let projection = a + t * ab;
        return (p - projection).length_squared();
    }
}

/// スケッチのジオメトリをGizmosで描画するシステム
fn draw_sketch_gizmos(
    mut gizmos: Gizmos,
    sketch_data: Res<SketchData>,
    active_tool: Res<ActiveSketchTool>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    // 新しいクエリ
    q_lines: Query<(&SketchLine, Option<&Selected>)>,
    q_circles: Query<(&SketchCircle, Option<&Selected>)>,
    q_rectangles: Query<(&SketchRectangle, Option<&Selected>)>,
) {
    // 完成した線を描画
    for (line, selected) in q_lines.iter() {
        let color = if selected.is_some() { Color::BLUE } else { Color::WHITE };
        gizmos.line(line.p1, line.p2, color);
    }
    // 完成した円を描画
    for (circle, selected) in q_circles.iter() {
        let color = if selected.is_some() { Color::BLUE } else { Color::WHITE };
        gizmos.circle(circle.center, Direction3d::Y, circle.radius, color);
    }
    // 完成した四角形を描画
    for (rect, selected) in q_rectangles.iter() {
        let color = if selected.is_some() { Color::BLUE } else { Color::WHITE };
        draw_rectangle(&mut gizmos, rect.p1, rect.p2, color);
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
                _ => {},
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