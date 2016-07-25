//! # Cursive
//!
//! [Cursive](https://github.com/gyscos/Cursive) is a TUI library built on top
//! of ncurses-rs.
//! It allows to easily build layouts for text-based applications.
//!
//! ## Getting started
//!
//! * Every application should start with a [`Cursive`](struct.Cursive.html)
//!   object. It is the main entry-point to the library.
//! * A declarative phase then describes the structure of the UI by adding
//!   views and configuring their behaviours.
//! * Finally, the event loop is started by calling
//!   [`Cursive::run(&mut self)`](struct.Cursive.html#method.run).
//!
//! ## Views
//!
//! Views are the main components of a cursive interface.
//! The [`view`](./view/index.html) module contains many views to use in your
//! application; if you don't find what you need, you may also implement the
//! [`View`](view/trait.View.html) trait and build your own.
//!
//! ## Callbacks
//!
//! Cursive is a *reactive* UI: it *reacts* to events generated by user input.
//!
//! During the declarative phase, callbacks are set to trigger on specific
//! events. These functions usually take a `&mut Cursive` argument, allowing
//! them to modify the view tree at will.
//!
//! ## Examples
//!
//! ```no_run
//! extern crate cursive;
//!
//! use cursive::prelude::*;
//!
//! fn main() {
//!     let mut siv = Cursive::new();
//!
//!     siv.add_layer(TextView::new("Hello World!\nPress q to quit."));
//!
//!     siv.add_global_callback('q', |s| s.quit());
//!
//!     siv.run();
//! }
//! ```
#![deny(missing_docs)]

extern crate ncurses;
extern crate toml;
extern crate unicode_segmentation;
extern crate unicode_width;

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        use ::std::io::Write;
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

macro_rules! new_default(
    ($c:ident) => {
        impl Default for $c {
            fn default() -> Self {
                Self::new()
            }
        }
    }
    );

pub mod prelude;

pub mod event;
pub mod view;
pub mod vec;
pub mod theme;
pub mod align;
pub mod menu;
pub mod direction;

// This probably doesn't need to be public?
mod printer;
mod xy;
mod with;

mod div;
mod utf8;

mod backend;

pub use xy::XY;
pub use with::With;
pub use printer::Printer;

use backend::{Backend, NcursesBackend};

use std::any::Any;
use std::collections::HashMap;
use std::path::Path;

use vec::Vec2;
use view::View;
use view::{Selector, StackView};

use event::{Callback, Event, EventResult};

/// Identifies a screen in the cursive ROOT.
pub type ScreenId = usize;

/// Central part of the cursive library.
///
/// It initializes ncurses on creation and cleans up on drop.
/// To use it, you should populate it with views, layouts and callbacks,
/// then start the event loop with run().
///
/// It uses a list of screen, with one screen active at a time.
pub struct Cursive {
    theme: theme::Theme,
    screens: Vec<StackView>,
    global_callbacks: HashMap<Event, Callback>,
    menubar: view::Menubar,

    active_screen: ScreenId,

    running: bool,
}

new_default!(Cursive);

// Use the Ncurses backend.
// TODO: make this feature-driven
type B = NcursesBackend;

impl Cursive {
    /// Creates a new Cursive root, and initialize ncurses.
    pub fn new() -> Self {
        // Default delay is way too long. 25 is imperceptible yet works fine.
        B::init();

        let theme = theme::load_default();
        // let theme = theme::load_theme("assets/style.toml").unwrap();

        let mut res = Cursive {
            theme: theme,
            screens: Vec::new(),
            global_callbacks: HashMap::new(),
            menubar: view::Menubar::new(),
            active_screen: 0,
            running: true,
        };

        res.screens.push(StackView::new());

        res
    }

    /// Selects the menubar
    pub fn select_menubar(&mut self) {
        self.menubar.take_focus(direction::Direction::none());
    }

    /// Sets the menubar autohide_menubar feature.
    ///
    /// * When enabled, the menu is only visible when selected.
    /// * When disabled, the menu is always visible and reserves the top row.
    pub fn set_autohide_menu(&mut self, autohide: bool) {
        self.menubar.autohide = autohide;
    }

    /// Retrieve the menu tree used by the menubar.
    ///
    /// This allows to add menu items to the menubar.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cursive;
    /// #
    /// # use cursive::prelude::*;
    /// #
    /// # fn main() {

    /// let mut siv = Cursive::new();
    ///
    /// siv.menubar()
    ///    .add("File",
    ///         MenuTree::new()
    ///             .leaf("New", |s| s.add_layer(Dialog::info("New file!")))
    ///             .subtree("Recent", MenuTree::new().with(|tree| {
    ///                 for i in 1..100 {
    ///                     tree.add_leaf(&format!("Item {}", i), |_| ())
    ///                 }
    ///             }))
    ///             .delimiter()
    ///             .with(|tree| {
    ///                 for i in 1..10 {
    ///                     tree.add_leaf(&format!("Option {}", i), |_| ());
    ///                 }
    ///             })
    ///             .delimiter()
    ///             .leaf("Quit", |s| s.quit()))
    ///    .add("Help",
    ///         MenuTree::new()
    ///             .subtree("Help",
    ///                      MenuTree::new()
    ///                          .leaf("General", |s| {
    ///                              s.add_layer(Dialog::info("Help message!"))
    ///                          })
    ///                          .leaf("Online", |s| {
    ///                              s.add_layer(Dialog::info("Online help?"))
    ///                          }))
    ///             .leaf("About",
    ///                   |s| s.add_layer(Dialog::info("Cursive v0.0.0"))));
    ///
    /// siv.add_global_callback(Key::Esc, |s| s.select_menubar());
    /// # }
    /// ```
    pub fn menubar(&mut self) -> &mut view::Menubar {
        &mut self.menubar
    }

    /// Returns the currently used theme
    pub fn current_theme(&self) -> &theme::Theme {
        &self.theme
    }

    /// Loads a theme from the given file.
    ///
    /// `filename` must point to a valid toml file.
    pub fn load_theme_file<P: AsRef<Path>>(&mut self, filename: P) -> Result<(), theme::Error> {
        self.theme = try!(theme::load_theme_file(filename));
        Ok(())
    }

    /// Loads a theme from the given string content.
    ///
    /// Content must be valid toml.
    pub fn load_theme(&mut self, content: &str) -> Result<(), theme::Error> {
        self.theme = try!(theme::load_theme(content));
        Ok(())
    }

    /// Sets the refresh rate, in frames per second.
    ///
    /// Regularly redraws everything, even when no input is given.
    /// Between 0 and 1000.
    /// Call with fps=0 to disable (default value).
    pub fn set_fps(&self, fps: u32) {
        B::set_refresh_rate(fps)
    }

    /// Returns a reference to the currently active screen.
    pub fn screen(&self) -> &StackView {
        let id = self.active_screen;
        &self.screens[id]
    }

    /// Returns a mutable reference to the currently active screen.
    pub fn screen_mut(&mut self) -> &mut StackView {
        let id = self.active_screen;
        self.screens.get_mut(id).unwrap()
    }

    /// Adds a new screen, and returns its ID.
    pub fn add_screen(&mut self) -> ScreenId {
        let res = self.screens.len();
        self.screens.push(StackView::new());
        res
    }

    /// Convenient method to create a new screen, and set it as active.
    pub fn add_active_screen(&mut self) -> ScreenId {
        let res = self.add_screen();
        self.set_screen(res);
        res
    }

    /// Sets the active screen. Panics if no such screen exist.
    pub fn set_screen(&mut self, screen_id: ScreenId) {
        if screen_id >= self.screens.len() {
            panic!("Tried to set an invalid screen ID: {}, but only {} \
                    screens present.",
                   screen_id,
                   self.screens.len());
        }
        self.active_screen = screen_id;
    }

    fn find_any(&mut self, selector: &Selector) -> Option<&mut Any> {
        // Internal find method that returns a Any object.
        self.screen_mut().find(selector)
    }

    /// Tries to find the view pointed to by the given path.
    ///
    /// If the view is not found, or if it is not of the asked type,
    /// it returns None.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cursive;
    /// # use cursive::prelude::*;
    /// # fn main() {
    /// let mut siv = Cursive::new();
    ///
    /// siv.add_layer(IdView::new("text", TextView::new("Text #1")));
    ///
    /// siv.add_global_callback('p', |s| {
    ///     s.find::<TextView>(&Selector::Id("text"))
    ///      .unwrap()
    ///      .set_content("Text #2");
    /// });
    /// # }
    /// ```
    pub fn find<V: View + Any>(&mut self, sel: &Selector) -> Option<&mut V> {
        match self.find_any(sel) {
            None => None,
            Some(b) => b.downcast_mut::<V>(),
        }
    }

    /// Convenient method to use `find` with a `Selector::Id`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cursive;
    /// # use cursive::prelude::*;
    /// # fn main() {
    /// let mut siv = Cursive::new();
    ///
    /// siv.add_layer(IdView::new("text", TextView::new("Text #1")));
    ///
    /// siv.add_global_callback('p', |s| {
    ///     s.find_id::<TextView>("text")
    ///      .unwrap()
    ///      .set_content("Text #2");
    /// });
    /// # }
    /// ```
    pub fn find_id<V: View + Any>(&mut self, id: &str) -> Option<&mut V> {
        self.find(&Selector::Id(id))
    }

    /// Adds a global callback.
    ///
    /// Will be triggered on the given key press when no view catches it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cursive;
    /// # use cursive::prelude::*;
    /// # fn main() {
    /// let mut siv = Cursive::new();
    ///
    /// siv.add_global_callback('q', |s| s.quit());
    /// # }
    /// ```
    pub fn add_global_callback<F, E: Into<Event>>(&mut self, event: E, cb: F)
        where F: Fn(&mut Cursive) + 'static
    {
        self.global_callbacks.insert(event.into(), Callback::from_fn(cb));
    }

    /// Convenient method to add a layer to the current screen.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate cursive;
    /// # use cursive::prelude::*;
    /// # fn main() {
    /// let mut siv = Cursive::new();
    ///
    /// siv.add_layer(TextView::new("Hello world!"));
    /// # }
    /// ```
    pub fn add_layer<T: 'static + View>(&mut self, view: T) {
        self.screen_mut().add_layer(view);
    }

    /// Convenient method to remove a layer from the current screen.
    pub fn pop_layer(&mut self) {
        self.screen_mut().pop_layer();
    }

    // Handles a key event when it was ignored by the current view
    fn on_event(&mut self, event: Event) {
        let cb = match self.global_callbacks.get(&event) {
            None => return,
            Some(cb) => cb.clone(),
        };
        // Not from a view, so no viewpath here
        cb(self);
    }

    /// Returns the size of the screen, in characters.
    pub fn screen_size(&self) -> Vec2 {
        let (x, y) = B::screen_size();

        Vec2 {
            x: x as usize,
            y: y as usize,
        }
    }

    fn layout(&mut self) {
        let size = self.screen_size();
        self.screen_mut().layout(size);
    }

    fn draw(&mut self) {
        // TODO: don't clone the theme
        // Reference it or something
        let printer = Printer::new(self.screen_size(), self.theme.clone());

        // Draw the currently active screen
        // If the menubar is active, nothing else can be.
        let offset = if self.menubar.autohide {
            0
        } else {
            1
        };
        // Draw the menubar?
        if self.menubar.visible() {
            let printer = printer.sub_printer(Vec2::zero(),
                                              printer.size,
                                              self.menubar.receive_events());
            self.menubar.draw(&printer);
        }

        let selected = self.menubar.receive_events();

        let printer =
            printer.sub_printer(Vec2::new(0, offset), printer.size, !selected);
        self.screen_mut().draw(&printer);

        B::refresh();
    }

    /// Runs the event loop.
    ///
    /// It will wait for user input (key presses)
    /// and trigger callbacks accordingly.
    ///
    /// Blocks until quit() is called.
    pub fn run(&mut self) {

        // And the big event loop begins!
        while self.running {
            // Do we need to redraw everytime?
            // Probably, actually.
            // TODO: Do we need to re-layout everytime?
            self.layout();

            // TODO: Do we need to redraw every view every time?
            // (Is this getting repetitive? :p)
            self.draw();

            // Wait for next event.
            // (If set_fps was called, this returns -1 now and then)
            let event = B::poll_event();
            if event == Event::WindowResize {
                B::clear();
            }

            // Event dispatch order:
            // * Focused element:
            //     * Menubar (if active)
            //     * Current screen (top layer)
            // * Global callbacks
            if self.menubar.receive_events() {
                if let EventResult::Consumed(Some(cb)) = self.menubar
                    .on_event(event) {
                    cb(self);
                }
            } else {
                match self.screen_mut().on_event(event) {
                    // If the event was ignored,
                    // it is our turn to play with it.
                    EventResult::Ignored => self.on_event(event),
                    EventResult::Consumed(None) => (),
                    EventResult::Consumed(Some(cb)) => cb(self),
                }
            }
        }
    }

    /// Stops the event loop.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

impl Drop for Cursive {
    fn drop(&mut self) {
        B::finish();
    }
}
