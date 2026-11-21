//! Accessibility semantics shared by the design-system components.
//!
//! GPUI takes a node's role from `Element::a11y_role` and its other properties from
//! `Element::write_a11y_info`, and reports only elements that have an id. It has no setter
//! for disabled, read-only, invalid, modal, busy or live state, so `annotate` edits the node
//! after GPUI has written its own properties.

use gpui::{
    A11ySubtreeBuilder, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Role, SharedString, Stateful,
    StatefulInteractiveElement, Window, accesskit::Node, div,
};
use std::{fmt, panic::Location};

/// An element whose node GPUI reports with `annotate` applied after its own properties.
pub struct Annotated<E> {
    element: E,
    annotate: Box<dyn Fn(&mut Node)>,
}

/// Wraps `element` so that `annotate` runs on the node GPUI builds for it.
pub fn annotate<E: Element>(element: E, annotate: impl Fn(&mut Node) + 'static) -> Annotated<E> {
    Annotated {
        element,
        annotate: Box::new(annotate),
    }
}

/// Marks `element` disabled for assistive technology when `disabled` is set.
pub fn disabled<E: Element>(element: E, disabled: bool) -> Annotated<E> {
    annotate(element, move |node| {
        if disabled {
            node.set_disabled();
        }
    })
}

/// Marks `element` busy (a loading control or region) when `busy` is set.
pub fn busy<E: Element>(element: E, busy: bool) -> Annotated<E> {
    annotate(element, move |node| {
        if busy {
            node.set_busy();
        }
    })
}

/// A plain element that carries `role` and `name` around `content`, for content that GPUI
/// animates: `with_animation` forwards no role, so the role sits on this frame instead. The
/// frame has no style of its own, so the content keeps its size and place as long as the
/// frame takes the content's place in its parent.
pub fn framed(
    id: ElementId,
    role: Role,
    name: Option<SharedString>,
    content: impl IntoElement,
) -> Stateful<Div> {
    let frame = div().id(id).role(role);
    let frame = match name {
        Some(name) => frame.aria_label(name),
        None => frame,
    };
    frame.child(content)
}

impl<E> fmt::Debug for Annotated<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Annotated").finish_non_exhaustive()
    }
}

impl<E: Element> Element for Annotated<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        self.element.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.element
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }

    fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
        self.element.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut Node) {
        self.element.write_a11y_info(node);
        (self.annotate)(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        self.element.a11y_synthetic_children(prepaint, builder);
    }
}

/// Reads the node GPUI would report for a rendered element, for tests.
///
/// The test platform never activates AccessKit, so `Window::debug_a11y_tree_json` stays `None`.
/// The probe makes the two calls GPUI makes for each element it reports: `a11y_role`, then
/// `write_a11y_info` on that same element, and only for an element that has an id.
#[cfg(test)]
pub(crate) mod probe {
    use gpui::{
        App, AppContext, Context, Element, Entity, IntoElement, Render, TestAppContext,
        VisualTestContext, Window, accesskit::Node, div,
    };
    use std::{cell::RefCell, rc::Rc};

    type Build = Box<dyn FnOnce(&mut Window, &mut App) -> Option<Node>>;

    /// Hosts the element under test in a view, which components need for their view state.
    struct Host {
        build: Option<Build>,
        node: Rc<RefCell<Option<Node>>>,
    }

    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if let Some(build) = self.build.take() {
                *self.node.borrow_mut() = build(window, cx);
            }
            div()
        }
    }

    /// The node GPUI reports for the element `element` renders to, or `None` when it reports none.
    pub(crate) fn reported(element: impl IntoElement) -> Option<Node> {
        let element = element.into_element();
        element.id()?;
        let role = element.a11y_role()?;
        let mut node = Node::new(role);
        element.write_a11y_info(&mut node);
        Some(node)
    }

    /// Reports the node of the element that the view built by `create` renders to.
    pub(crate) fn view<V: Render + 'static>(
        cx: &mut TestAppContext,
        create: impl FnOnce(&mut Context<V>) -> V,
    ) -> Option<Node> {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let entity: Entity<V> = cx.new(create);
        root(cx, move |window, cx| {
            entity.update(cx, |view, cx| reported(view.render(window, cx)))
        })
    }

    /// Draws `build` in a test window with the design system installed and returns its node.
    pub(crate) fn root(
        cx: &mut TestAppContext,
        build: impl FnOnce(&mut Window, &mut App) -> Option<Node> + 'static,
    ) -> Option<Node> {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let node = Rc::new(RefCell::new(None));
        let host = {
            let node = node.clone();
            cx.add_window(move |_, _| Host {
                build: Some(Box::new(build)),
                node,
            })
        };
        let cx = &mut VisualTestContext::from_window(host.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        node.replace(None)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        annotate, busy, disabled,
        probe::{reported, root},
    };
    use gpui::{
        Div, InteractiveElement, Stateful, StatefulInteractiveElement, TestAppContext,
        accesskit::Role, div,
    };

    #[gpui::test]
    fn the_test_platform_builds_no_tree(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, _| {
            assert!(window.is_a11y_enabled());
            assert!(!window.is_a11y_active());
            assert!(window.debug_a11y_tree_json().is_none());
        });
    }

    #[gpui::test]
    fn an_element_with_an_id_but_no_role_is_not_reported(cx: &mut TestAppContext) {
        assert!(root(cx, |_, _| reported(div().id("plain"))).is_none());
    }

    #[gpui::test]
    fn a_role_is_reported_on_the_element_it_is_set_on(cx: &mut TestAppContext) {
        let node = root(cx, |_, _| reported(div().id("save").role(Role::Button)));
        assert_eq!(node.map(|node| node.role()), Some(Role::Button));
    }

    #[gpui::test]
    fn annotations_run_after_gpui_writes_its_own_properties(cx: &mut TestAppContext) {
        let node = root(cx, |_, _| {
            reported(annotate(
                div().id("save").role(Role::Button).aria_label("Save"),
                |node| node.set_read_only(),
            ))
        })
        .expect("the annotated element is reported");
        assert_eq!(node.label(), Some("Save"));
        assert!(node.is_read_only());
    }

    #[gpui::test]
    fn disabled_and_busy_mark_the_node_only_when_set(cx: &mut TestAppContext) {
        fn save_button() -> Stateful<Div> {
            div().id("save").role(Role::Button)
        }
        let idle = root(cx, |_, _| {
            reported(disabled(busy(save_button(), false), false))
        })
        .expect("reported");
        assert!(!idle.is_disabled() && !idle.is_busy());
        let stuck = root(cx, |_, _| {
            reported(disabled(busy(save_button(), true), true))
        })
        .expect("reported");
        assert!(stuck.is_disabled() && stuck.is_busy());
    }
}

#[cfg(test)]
mod frame_tests {
    use super::{
        ElementId, framed,
        probe::{reported, root},
    };
    use gpui::{TestAppContext, accesskit::Role, div};

    #[gpui::test]
    fn a_frame_carries_the_role_and_name_of_the_content_it_holds(cx: &mut TestAppContext) {
        let node = root(cx, |_, _| {
            reported(framed(
                ElementId::from("toast"),
                Role::Status,
                Some("Saved".into()),
                div(),
            ))
        })
        .expect("the frame is reported");
        assert_eq!(node.role(), Role::Status);
        assert_eq!(node.label(), Some("Saved"));
    }
}
