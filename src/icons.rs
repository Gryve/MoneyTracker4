//! Icons: Lucide-style 24×24 outlines, rasterised by Rune from their SVG bodies.
//!
//! `currentColor` takes the colour the element draws with, so one icon serves every token.

use rune::Icon;

macro_rules! icon {
    ($name:ident, $id:literal, $body:literal) => {
        pub static $name: Icon = Icon {
            name: $id,
            view: 24.0,
            body: $body,
        };
    };
}

// ── Navigation ──────────────────────────────────────────────────────────────

icon!(
    DASHBOARD,
    "mt-dashboard",
    r#"<path d="M4 4h6v7H4zM14 4h6v4h-6zM4 15h6v5H4zM14 12h6v8h-6z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/>"#
);
icon!(
    TRANSACTIONS,
    "mt-transactions",
    r#"<path d="M4 8h14M15 5l3 3-3 3M20 16H6M9 13l-3 3 3 3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CALENDAR,
    "mt-calendar",
    r#"<path d="M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/><path d="M4 9h16M8 3v4M16 3v4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#
);
icon!(
    BUDGETS,
    "mt-budgets",
    r#"<circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="12" cy="12" r="4.2" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/>"#
);
icon!(
    YEAR,
    "mt-year",
    r#"<path d="M3 20h18M6 20V11M12 20V4M18 20v-6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#
);

// ── Actions ─────────────────────────────────────────────────────────────────

icon!(
    PLUS,
    "mt-plus",
    r#"<path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#
);
icon!(
    TRASH,
    "mt-trash",
    r#"<path d="M4 7h16M9.5 7V5h5v2M6.5 7l1 13h9l1-13M10.5 11v5M13.5 11v5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    PENCIL,
    "mt-pencil",
    r#"<path d="M4 20h4L20 8l-4-4L4 16z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    CHEVRON_LEFT,
    "mt-chevron-left",
    r#"<path d="M14.5 5.5L8 12l6.5 6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CHEVRON_RIGHT,
    "mt-chevron-right",
    r#"<path d="M9.5 5.5L16 12l-6.5 6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CHECK,
    "mt-check",
    r#"<path d="M5 12.5l4.5 4.5L19 7" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CLOSE,
    "mt-close",
    r#"<path d="M6 6l12 12M18 6L6 18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#
);
icon!(
    ALERT,
    "mt-alert",
    r#"<path d="M12 4.5L2.8 20h18.4z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/><path d="M12 10v4.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><circle cx="12" cy="17.2" r="0.95" fill="currentColor"/>"#
);
icon!(
    SEARCH,
    "mt-search",
    r#"<circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M15.4 15.4L20 20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#
);

// ── Money ───────────────────────────────────────────────────────────────────

icon!(
    ARROW_IN,
    "mt-arrow-in",
    r#"<path d="M12 19V5M6.5 10.5L12 5l5.5 5.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    ARROW_OUT,
    "mt-arrow-out",
    r#"<path d="M12 5v14M6.5 13.5L12 19l5.5-5.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    SCALE,
    "mt-scale",
    r#"<path d="M12 4v16M6 8h12M6 8l-3 6h6zM18 8l-3 6h6z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);

// ── Categories ──────────────────────────────────────────────────────────────

icon!(
    FOOD,
    "mt-food",
    r#"<path d="M6 3v7a2.2 2.2 0 0 0 4.4 0V3M8.2 10.2V21M16.5 3c1.8 1.4 2 3.4 2 5s-.6 2.6-2 3.2V21" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    HOME,
    "mt-home",
    r#"<path d="M4 10.5L12 4l8 6.5V20a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    CAR,
    "mt-car",
    r#"<path d="M4 13l2-5h12l2 5v4h-2M4 17H2v-4M4 13h16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="7" cy="17" r="1.7" fill="none" stroke="currentColor" stroke-width="1.6"/><circle cx="17" cy="17" r="1.7" fill="none" stroke="currentColor" stroke-width="1.6"/>"#
);
icon!(
    CART,
    "mt-cart",
    r#"<circle cx="9" cy="20" r="1.4" fill="none" stroke="currentColor" stroke-width="1.6"/><circle cx="18" cy="20" r="1.4" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M3 3h2l2.5 11h11L21 7H6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    HEALTH,
    "mt-health",
    r#"<path d="M12 20s-7-4.4-7-9a4 4 0 0 1 7-2.6A4 4 0 0 1 19 11c0 4.6-7 9-7 9z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    TICKET,
    "mt-ticket",
    r#"<path d="M4 8a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2a2 2 0 0 0 0 4v2a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-2a2 2 0 0 0 0-4z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><path d="M10 6.5v11" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>"#
);
icon!(
    SHIRT,
    "mt-shirt",
    r#"<path d="M8.5 3L4 6l2 3 2.5-1.2V21h7V7.8L18 9l2-3-4.5-3-1.8 2h-3.4z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    BOOK,
    "mt-book",
    r#"<path d="M4 5.5A1.5 1.5 0 0 1 5.5 4H11v17H5.5A1.5 1.5 0 0 1 4 19.5zM20 5.5A1.5 1.5 0 0 0 18.5 4H13v17h5.5a1.5 1.5 0 0 0 1.5-1.5z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    BULB,
    "mt-bulb",
    r#"<path d="M9.5 18h5M10.5 21h3M12 3a6 6 0 0 0-3.6 10.8V16h7.2v-2.2A6 6 0 0 0 12 3z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    PHONE,
    "mt-phone",
    r#"<path d="M8 3h8a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM11 18h2" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    GIFT,
    "mt-gift",
    r#"<path d="M4 11h16v9H4zM4 11V8h16v3M12 11v9M12 8S10 3.4 8.2 4.6 9.6 8 12 8zM12 8s2-4.6 3.8-3.4S14.4 8 12 8z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    PLANE,
    "mt-plane",
    r#"<path d="M12 2.8c1 0 1.6 1.1 1.6 2.7v4.2l7 4.4v2l-7-2v3.6l2.4 2v1.5L12 19.7l-4 1.5v-1.5l2.4-2v-3.6l-7 2v-2l7-4.4V5.5c0-1.6.6-2.7 1.6-2.7z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>"#
);
icon!(
    WRENCH,
    "mt-wrench",
    r#"<path d="M14.8 3a5 5 0 0 0-4.2 7.6L3 18.2 5.8 21l7.6-7.6A5 5 0 0 0 20.6 7l-2.9 2.9-2.6-2.6L18 4.4A5 5 0 0 0 14.8 3z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>"#
);
icon!(
    BRIEFCASE,
    "mt-briefcase",
    r#"<path d="M3.5 8h17v12h-17zM8.5 8V5.5h7V8M3.5 13h17" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    WALLET,
    "mt-wallet",
    r#"<path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h11A2.5 2.5 0 0 1 19 7.5V9H5.5A2.5 2.5 0 0 0 3 11.5zM3 9.5v7A2.5 2.5 0 0 0 5.5 19h13A2.5 2.5 0 0 0 21 16.5V13h-5a2 2 0 0 1 0-4h5V9z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>"#
);
icon!(
    LAPTOP,
    "mt-laptop",
    r#"<path d="M5 6h14v10H5zM2.5 19h19" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    TRENDING,
    "mt-trending",
    r#"<path d="M3 17l6-6 4 4 7-7M15 8h6v6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    BANK,
    "mt-bank",
    r#"<path d="M3 10l9-6 9 6M4.5 10v8M9.5 10v8M14.5 10v8M19.5 10v8M3 21h18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    PACKAGE,
    "mt-package",
    r#"<path d="M3 8l9-5 9 5v8l-9 5-9-5zM3 8l9 5 9-5M12 13v8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#
);
icon!(
    REFUND,
    "mt-refund",
    r#"<path d="M20 12a8 8 0 1 1-3.2-6.4M20 4v5h-5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    COIN,
    "mt-coin",
    r#"<circle cx="12" cy="12" r="8.8" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M12 7v10M15 9.4c0-1.3-1.3-2-3-2s-3 .7-3 2 1.3 1.9 3 2.2 3 .9 3 2.2-1.3 2-3 2-3-.7-3-2" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>"#
);
icon!(
    TAG,
    "mt-tag",
    r#"<path d="M11 3H3v8l10 10 8-8z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><circle cx="7.5" cy="7.5" r="1.3" fill="currentColor"/>"#
);

/// The icon a category shows: matched on its display name (the stored name without the emoji),
/// with a per-kind fallback for categories the user adds.
pub fn for_category(display: &str, kind: crate::model::Kind) -> &'static Icon {
    let name = display.to_ascii_lowercase();
    let icon = if name.contains("food") || name.contains("dining") || name.contains("restaurant") {
        &FOOD
    } else if name.contains("hous") || name.contains("rent") || name.contains("home") {
        &HOME
    } else if name.contains("transport") || name.contains("car") || name.contains("fuel") {
        &CAR
    } else if name.contains("grocer") || name.contains("market") {
        &CART
    } else if name.contains("health") || name.contains("medical") || name.contains("doctor") {
        &HEALTH
    } else if name.contains("entertain") || name.contains("fun") || name.contains("cinema") {
        &TICKET
    } else if name.contains("cloth") || name.contains("wear") {
        &SHIRT
    } else if name.contains("education") || name.contains("school") || name.contains("course") {
        &BOOK
    } else if name.contains("utilit")
        || name.contains("electric")
        || name.contains("gas")
        || name.contains("water")
    {
        &BULB
    } else if name.contains("subscription")
        || name.contains("phone")
        || name.contains("internet")
        || name.contains("mobile")
    {
        &PHONE
    } else if name.contains("gift") {
        &GIFT
    } else if name.contains("travel") || name.contains("flight") || name.contains("holiday") {
        &PLANE
    } else if name.contains("maintenance") || name.contains("repair") {
        &WRENCH
    } else if name.contains("salary") || name.contains("wage") {
        &WALLET
    } else if name.contains("freelance") || name.contains("contract") {
        &LAPTOP
    } else if name.contains("invest") || name.contains("stock") || name.contains("crypto") {
        &TRENDING
    } else if name.contains("interest") || name.contains("dividend") {
        &BANK
    } else if name.contains("refund") || name.contains("return") {
        &REFUND
    } else if name.contains("business") {
        &BRIEFCASE
    } else if name.contains("other") {
        &COIN
    } else {
        match kind {
            crate::model::Kind::Income => &COIN,
            crate::model::Kind::Expense => &PACKAGE,
        }
    };
    icon
}
