use gtk4 as gtk;
use gtk::prelude::*;

pub fn show_shortcuts_dialog(parent: &impl IsA<gtk::Window>) {
    let dialog = gtk::ShortcutsWindow::builder()
        .modal(true)
        .transient_for(parent)
        .build();

    let section = gtk::ShortcutsSection::builder()
        .section_name("shortcuts")
        .build();

    // 1. Playback & Transport Group
    let group_playback = gtk::ShortcutsGroup::builder()
        .title("Playback & Transport")
        .build();

    let sc_play = gtk::ShortcutsShortcut::builder()
        .title("Play / Pause")
        .accelerator("space")
        .build();

    let sc_seek_bwd = gtk::ShortcutsShortcut::builder()
        .title("Seek 5 seconds backward")
        .accelerator("Left")
        .build();

    let sc_seek_fwd = gtk::ShortcutsShortcut::builder()
        .title("Seek 5 seconds forward")
        .accelerator("Right")
        .build();

    let sc_next = gtk::ShortcutsShortcut::builder()
        .title("Next Track")
        .accelerator("<Shift>N")
        .build();

    let sc_prev = gtk::ShortcutsShortcut::builder()
        .title("Previous Track")
        .accelerator("<Shift>P")
        .build();

    group_playback.append(&sc_play);
    group_playback.append(&sc_seek_bwd);
    group_playback.append(&sc_seek_fwd);
    group_playback.append(&sc_next);
    group_playback.append(&sc_prev);

    // 2. Navigation & Layout Group
    let group_nav = gtk::ShortcutsGroup::builder()
        .title("Navigation & Layout")
        .build();

    let sc_search = gtk::ShortcutsShortcut::builder()
        .title("Focus Search")
        .accelerator("<Ctrl>F")
        .build();

    let sc_sidebar = gtk::ShortcutsShortcut::builder()
        .title("Toggle Sidebar (Collapse / Expand)")
        .accelerator("<Ctrl>B")
        .build();

    let sc_mini = gtk::ShortcutsShortcut::builder()
        .title("Toggle Compact Mini-Player")
        .accelerator("<Ctrl>M")
        .build();

    let sc_help = gtk::ShortcutsShortcut::builder()
        .title("Keyboard Shortcuts Cheat Sheet")
        .accelerator("F1")
        .build();

    let sc_quit = gtk::ShortcutsShortcut::builder()
        .title("Quit Application")
        .accelerator("<Ctrl>Q")
        .build();

    group_nav.append(&sc_search);
    group_nav.append(&sc_sidebar);
    group_nav.append(&sc_mini);
    group_nav.append(&sc_help);
    group_nav.append(&sc_quit);

    section.append(&group_playback);
    section.append(&group_nav);
    dialog.set_child(Some(&section));

    dialog.present();
}
