use crate::{
    commands,
    data::{AudioDuration, Navigation, Playback, State, Track},
    ui::{album, theme},
    widgets::{icons, HoverExt, Maybe},
};
use druid::{
    lens::{Id, InArc},
    widget::{Controller, CrossAxisAlignment, Flex, Label, Painter, SizedBox, ViewSwitcher},
    Color, Env, Event, EventCtx, MouseButton, MouseEvent, PaintCtx, Point, Rect, RenderContext,
    Size, Widget, WidgetExt,
};
use std::sync::Arc;

pub fn make_panel() -> impl Widget<State> {
    Flex::row()
        .with_flex_child(make_info().align_left(), 1.0)
        .with_flex_child(make_player().align_right(), 1.0)
        .expand_width()
        .padding(theme::grid(1.0))
        .background(theme::WHITE)
        .lens(State::playback)
}

fn make_info() -> impl Widget<Playback> {
    Maybe::or_empty(make_info_track).lens(Playback::item)
}

fn make_info_track() -> impl Widget<Arc<Track>> {
    let album_cover = Maybe::or_empty(|| album::make_cover(theme::grid(7.0), theme::grid(7.0)))
        .lens(Track::album);

    let track_name = Label::raw()
        .with_font(theme::UI_FONT_MEDIUM)
        .lens(Track::name);

    let track_artist = Label::dynamic(|track: &Track, _| track.artist_name())
        .with_text_size(theme::TEXT_SIZE_SMALL)
        .hover()
        .on_click(|ctx: &mut EventCtx, track: &mut Track, _| {
            if let Some(artist) = track.artists.front() {
                let nav = Navigation::ArtistDetail(artist.id.clone());
                ctx.submit_command(commands::NAVIGATE_TO.with(nav));
            }
        });

    let track_album = Label::dynamic(|track: &Track, _| track.album_name())
        .with_text_size(theme::TEXT_SIZE_SMALL)
        .hover()
        .on_click(|ctx, track: &mut Track, _| {
            if let Some(album) = track.album.as_ref() {
                let nav = Navigation::AlbumDetail(album.id.clone());
                ctx.submit_command(commands::NAVIGATE_TO.with(nav));
            }
        });

    let track_info = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(track_name)
        .with_child(track_artist)
        .with_child(track_album);

    Flex::row()
        .with_child(album_cover)
        .with_default_spacer()
        .with_child(track_info)
        .lens(InArc::new::<Arc<Track>, Arc<Track>>(Id))
}

fn make_player() -> impl Widget<Playback> {
    ViewSwitcher::new(
        |playback: &Playback, _| playback.item.is_some(),
        |&has_item, _, _| {
            if has_item {
                Flex::column()
                    .with_child(make_player_controls())
                    .with_default_spacer()
                    .with_child(make_player_progress())
                    .boxed()
            } else {
                SizedBox::empty().boxed()
            }
        },
    )
}

fn make_player_controls() -> impl Widget<Playback> {
    let play_previous = icons::SKIP_BACK
        .scale((theme::grid(2.0), theme::grid(2.0)))
        .padding(theme::grid(1.0))
        .hover()
        .on_click(|ctx, _, _| ctx.submit_command(commands::PLAY_PREVIOUS));

    let play_pause = ViewSwitcher::new(
        |playback: &Playback, _| playback.is_playing,
        |&is_playing, _, _| {
            if is_playing {
                icons::PAUSE
                    .scale((theme::grid(2.0), theme::grid(2.0)))
                    .padding(theme::grid(1.0))
                    .hover()
                    .on_click(|ctx, _, _| ctx.submit_command(commands::PLAY_PAUSE))
                    .boxed()
            } else {
                icons::PLAY
                    .scale((theme::grid(2.0), theme::grid(2.0)))
                    .padding(theme::grid(1.0))
                    .hover()
                    .on_click(|ctx, _, _| ctx.submit_command(commands::PLAY_RESUME))
                    .boxed()
            }
        },
    );

    let play_next = icons::SKIP_FORWARD
        .scale((theme::grid(2.0), theme::grid(2.0)))
        .padding(theme::grid(1.0))
        .hover()
        .on_click(|ctx, _, _| ctx.submit_command(commands::PLAY_NEXT));

    Flex::row()
        .with_child(play_previous)
        .with_child(play_pause)
        .with_child(play_next)
}

fn make_player_progress() -> impl Widget<Playback> {
    let current_time = Maybe::or_empty(|| {
        Label::dynamic(|progress: &AudioDuration, _| progress.as_minutes_and_seconds())
            .with_text_size(12.0)
            .align_right()
            .fix_width(theme::grid(4.0))
    })
    .lens(Playback::progress);
    let total_time = Maybe::or_empty(|| {
        Label::dynamic(|track: &Track, _| track.duration.as_minutes_and_seconds())
            .with_text_size(12.0)
            .align_left()
            .fix_width(theme::grid(4.0))
            .lens(InArc::new::<Arc<Track>, _>(Id))
    })
    .lens(Playback::item);
    Flex::row()
        .with_child(current_time)
        .with_default_spacer()
        .with_flex_child(make_volume_analysis(), 1.0)
        .with_default_spacer()
        .with_child(total_time)
}

fn make_volume_analysis() -> impl Widget<Playback> {
    Painter::new(|ctx, playback: &Playback, env| {
        if playback.analysis.is_some() {
            paint_progress_with_analysis(ctx, &playback, env);
        } else {
            paint_progress(ctx, &playback, env);
        }
    })
    .controller(SeekController)
    .fix_height(theme::grid(1.0))
}

const PROGRESS_MIN_SEGMENT_WIDTH: f64 = 1.0;
const PROGRESS_MIN_SEGMENT_HEIGHT: f64 = 3.0;

fn paint_progress_with_analysis(ctx: &mut PaintCtx, playback: &Playback, _env: &Env) {
    let analysis = playback.analysis.as_ref().unwrap();

    let elapsed_time = playback
        .progress
        .map(|progress| progress.as_secs_f64())
        .unwrap_or(0.0);
    let total_time = playback
        .item
        .as_ref()
        .map(|track| track.duration.as_secs_f64())
        .unwrap_or(0.0);

    let (min_loudness, max_loudness) = analysis.get_minmax_loudness();

    let elapsed_color = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let remaining_color = Color::rgba(0.3, 0.3, 0.3, 1.0);
    let bounds = ctx.size();
    for segment in &analysis.segments {
        let start_frac = segment.start.as_secs_f64() / total_time;
        let duration_frac = segment.duration.as_secs_f64() / total_time;
        let loudness_frac =
            (segment.loudness_max + min_loudness.abs()) / (max_loudness + min_loudness.abs());

        let size = Size::new(
            (bounds.width * duration_frac as f64).max(PROGRESS_MIN_SEGMENT_WIDTH),
            bounds.height * loudness_frac as f64,
        );
        let point = Point::new(
            bounds.width * start_frac as f64,
            bounds.height / 2.0 - size.height / 2.0,
        );
        ctx.fill(
            &Rect::from_origin_size(point, size),
            if segment.start.as_secs_f64() <= elapsed_time {
                &elapsed_color
            } else {
                &remaining_color
            },
        );
    }
}

fn paint_progress(ctx: &mut PaintCtx, playback: &Playback, env: &Env) {
    let elapsed_time = playback
        .progress
        .map(|progress| progress.as_secs_f32())
        .unwrap_or(0.0);
    let total_time = playback
        .item
        .as_ref()
        .map(|track| track.duration.as_secs_f32())
        .unwrap_or(0.0);

    let elapsed_color = env.get(theme::PRIMARY_DARK);
    let remaining_color = env.get(theme::PRIMARY_LIGHT).with_alpha(0.5);
    let bounds = ctx.size();

    let elapsed_frac = elapsed_time / total_time;
    let elapsed_width = bounds.width * elapsed_frac as f64;
    let remaining_width = bounds.width - elapsed_width;
    let elapsed = Size::new(elapsed_width, PROGRESS_MIN_SEGMENT_HEIGHT).round();
    let remaining = Size::new(remaining_width, PROGRESS_MIN_SEGMENT_HEIGHT).round();

    let vertical_center = bounds.height / 2.0 - PROGRESS_MIN_SEGMENT_HEIGHT / 2.0;
    ctx.fill(
        &Rect::from_origin_size(Point::new(0.0, vertical_center), elapsed),
        &elapsed_color,
    );
    ctx.fill(
        &Rect::from_origin_size(Point::new(elapsed.width, vertical_center), remaining),
        &remaining_color,
    );
}

struct SeekController;

impl<T, W: Widget<T>> Controller<T, W> for SeekController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let seek_to_mouse_pos = |ctx: &mut EventCtx, mouse_event: &MouseEvent| {
            let frac = mouse_event.pos.x / ctx.size().width;
            ctx.submit_command(commands::SEEK_TO_FRACTION.with(frac));
        };

        match event {
            Event::MouseDown(mouse_event) => {
                if mouse_event.button == MouseButton::Left {
                    ctx.set_active(true);
                }
            }
            Event::MouseUp(mouse_event) => {
                if ctx.is_active() && mouse_event.button == MouseButton::Left {
                    if ctx.is_hot() {
                        seek_to_mouse_pos(ctx, mouse_event);
                    }
                    ctx.set_active(false);
                }
            }
            _ => {}
        }
        child.event(ctx, event, data, env);
    }
}