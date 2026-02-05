# Luban Design System

## Overview

Luban's UI follows a clean, minimal design with a floating content panel layout. The design emphasizes clarity, consistency, and professional aesthetics.

## Color System

### Background Colors

| Token | Value | Usage |
|-------|-------|-------|
| `page-bg` | `#f5f5f5` | Page/window background |
| `surface` | `#fcfcfc` | Content panel background |
| `surface-hover` | `#f7f7f7` | Hover state for items |

### Border Colors

| Token | Value | Usage |
|-------|-------|-------|
| `border` | `#ebebeb` | Default borders |
| `border-subtle` | `#e5e5e5` | Dropdown borders |

### Text Colors

| Token | Value | Usage |
|-------|-------|-------|
| `text-primary` | `#1b1b1b` | Primary text, titles |
| `text-secondary` | `#6b6b6b` | Secondary text, icons |
| `text-muted` | `#9b9b9b` | Muted text, timestamps |

### Interactive States

| Token | Value | Usage |
|-------|-------|-------|
| `active` | `#e8e8e8` | Active/selected items |
| `hover` | `#eeeeee` | Hover state |
| `primary` | `#5e6ad2` | Primary accent, checkmarks |

### Status Colors

| Token | Value | Usage |
|-------|-------|-------|
| `status-success` | `#5e6ad2` | Completed, success |
| `status-warning` | `#f2994a` | In progress, needs review |
| `status-error` | `#eb5757` | Failed, error |

## Typography

### Font Sizes

| Size | Value | Usage |
|------|-------|-------|
| `xs` | `11px` | Badges, uppercase labels |
| `sm` | `12px` | Secondary text, timestamps |
| `base` | `13px` | Primary text, navigation |
| `lg` | `14px` | Page titles, headers |

### Font Weights

- `normal` (400): Body text
- `medium` (500): Headers, active items
- `semibold` (600): Project icons, workspace name

## Layout

### Main Layout Structure

```
┌─────────────────────────────────────────────────────┐
│ #f5f5f5 background                                  │
│  ┌──────────┐  ┌────────────────────────────────┐  │
│  │ Sidebar  │  │  Floating Content Panel        │  │
│  │ 244px    │  │  #fcfcfc                        │  │
│  │          │  │  margin: 8px 8px 8px 0          │  │
│  │          │  │  border-radius: 4px             │  │
│  │          │  │  box-shadow: subtle             │  │
│  └──────────┘  └────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### Dimensions

| Element | Value |
|---------|-------|
| Sidebar width | `244px` |
| Header height (content) | `39px` |
| Header height (sidebar) | `52px` |
| Content margin | `8px 8px 8px 0` |
| Content border-radius | `4px` |
| Content box-shadow | `rgba(0,0,0,0.022) 0 3px 6px -2px, rgba(0,0,0,0.044) 0 1px 1px 0` |

### Spacing

- Navigation item padding: `px-2 py-1.5`
- Section padding: `px-2 py-2`
- Content padding: `p-6` or `p-8`
- Gap between items: `0.5` (2px)

## Components

### Navigation Item

```tsx
<button className={cn(
  "w-full flex items-center gap-2 px-2 py-1.5 rounded text-[13px] transition-colors",
  active ? "bg-[#e8e8e8]" : "hover:bg-[#eeeeee]"
)} style={{ color: '#1b1b1b' }}>
  <Icon style={{ color: '#6b6b6b' }} />
  <span>{label}</span>
</button>
```

### Project Icon

```tsx
<span className={cn(
  "w-[14px] h-[14px] rounded-[3px] flex items-center justify-center",
  "text-[9px] font-semibold text-white",
  color // e.g., "bg-violet-500"
)}>
  {name.charAt(0).toUpperCase()}
</span>
```

### Header

```tsx
<div
  className="flex items-center justify-between h-[39px] flex-shrink-0"
  style={{ padding: '0 20px', borderBottom: '1px solid #ebebeb' }}
>
  {/* Content */}
</div>
```

### Badge

```tsx
<span
  className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0"
  style={{ backgroundColor: '#f0f0f0', color: '#6b6b6b' }}
>
  {text}
</span>
```

### Toggle Switch

```tsx
<button className={cn(
  "relative w-9 h-5 rounded-full transition-colors",
  enabled ? "bg-[#5e6ad2]" : "bg-[#d4d4d4]"
)}>
  <div className={cn(
    "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform",
    enabled ? "translate-x-4" : "translate-x-0.5"
  )} />
</button>
```

## Interaction Patterns

### Click Behavior

- **Single click**: Select/preview item
- **Double click**: Open full view

### Dropdown Menus

Use Radix UI `DropdownMenu` component with these styles:

```tsx
<DropdownMenuContent
  className="w-[240px] rounded-lg border-[#e5e5e5] bg-white shadow-[0_4px_16px_rgba(0,0,0,0.12)] p-1.5"
>
  <DropdownMenuItem
    className="flex items-center gap-2.5 px-2.5 py-2 text-[13px] rounded-md cursor-pointer hover:bg-[#f5f5f5]"
    style={{ color: '#1b1b1b' }}
  >
    <Icon style={{ color: '#6b6b6b' }} />
    {label}
  </DropdownMenuItem>
</DropdownMenuContent>
```

## Activity-Based Task View (Linear Style)

The activity-based task view follows Linear's issue detail page pattern. Styles extracted via agent-browser inspection of Linear app.

### Layout Structure

```
┌────────────────────────────────────────────────────────────────────────┐
│  ← 60px margin →│← max 686px content →│← 60px margin →                 │
├─────────────────┼─────────────────────┼───────────────                 │
│                 │ Task Title          │                                 │
│                 │ Task Description    │                                 │
│                 ├─────────────────────┤                                 │
│                 │ Activity            │                                 │
│                 ├─────────────────────┤                                 │
│                 │ [18px Avatar] user · 2mo ago                          │
│                 │ User message content...                               │
│                 ├─────────────────────┤                                 │
│                 │ [18px Avatar] Agent · 1h ago · 5s                     │
│                 │ ▶ Completed 5 steps                                   │
│                 │ Agent response content...                             │
│                 ├─────────────────────┤                                 │
│                 │ [Input Composer]                                      │
└─────────────────┴─────────────────────┴───────────────                 ┘
```

### Layout Dimensions (from Linear)

| Element | Value |
|---------|-------|
| Content max-width | `686px` |
| Content margin | `0 60px` |
| Comment padding | `16px 0px` (vertical only, horizontal from parent margin) |

### Typography (from Linear)

| Element | Size | Weight | Color |
|---------|------|--------|-------|
| Task title | 24px | 600 | #1b1b1b |
| Title line-height | 38.4px | - | - |
| Title letter-spacing | -0.1px | - | - |
| Description | 15px | 450 | #2f2f2f |
| Activity header | 15px | 600 | #1b1b1b |
| User/Agent name | 12px | 500 | #5b5b5d (muted) |
| Timestamp | 12px | 450 | #5b5b5d (muted) |
| Message content | 15px | 450 | #2f2f2f |
| Activity step | 12px | 450 | #5b5b5d |

### Comment Header Layout

| Element | Value |
|---------|-------|
| Avatar size | 20x20px, border-radius: 4px |
| Gap avatar to username | 11px |
| Gap between username/timestamp | 8px |

### Comment/Activity Item (from Linear)

| Element | Value |
|---------|-------|
| Border | **NONE** - comments have no border |
| Background | **transparent** - no card background |
| Box shadow | **NONE** |
| Timeline line | 1px solid #c8c8c8 (vertical connector) |
| Avatar position | **OUTSIDE** content, on the left |
| Avatar to content gap | 11px |

### Components

#### Activity Event (User Message)
```tsx
<div className="group/activity flex items-start">
  {/* Avatar - 20px with 4px border-radius, OUTSIDE content */}
  <div style={{ 
    width: '20px', 
    height: '20px', 
    marginRight: '11px',
    borderRadius: '4px', 
    backgroundColor: '#5e6ad2' 
  }}>U</div>
  
  {/* Content - NO border, NO background, NO shadow */}
  <div className="flex-1">
    {/* Header: name + timestamp */}
    <div className="flex items-center gap-2 mb-1">
      <span style={{ fontSize: '15px', fontWeight: 500, color: '#1b1b1b' }}>You</span>
      <span style={{ fontSize: '14px', fontWeight: 400, color: '#5b5b5d' }}>3m ago</span>
    </div>
    <div style={{ fontSize: '15px', fontWeight: 400, lineHeight: '22.5px', color: '#1b1b1b' }}>
      {/* Message content */}
    </div>
  </div>
</div>
```

#### Simple Event (System Event)
```tsx
<div className="flex items-start" style={{ padding: '1px 0' }}>
  {/* Icon column - 14x14, aligned with card avatars */}
  <div style={{ width: '14px', height: '16.8px', marginLeft: '14px', marginRight: '4px' }}>
    <div style={{ width: '14px', height: '14px', borderRadius: '50%', backgroundColor: '#5e6ad2' }}>
      {/* Icon or initial */}
    </div>
  </div>
  
  {/* Event text - inline, 12px, muted */}
  <span style={{ fontSize: '12px', color: '#5b5b5d' }}>
    <b style={{ fontWeight: 500 }}>wyatt</b> created the issue · 3mo ago
  </span>
</div>
```

#### Activity Icon Alignment (Critical)

In Linear-style activity lists, icon alignment is a primary visual anchor.

- Icon size: `14x14` (e.g. Tailwind `w-3.5 h-3.5`)
- Icon slot: `width: 14px`, `height: 16.8px`
- Text line-height: `16.8px`
- Rule: the icon center must be vertically centered with the text center for single-line rows
- Rule: simple events and card avatars share the same center X axis in the activity stream
- When rendering nested activity rows inside a card, align the activity icon center X with the card avatar center X
- Keep right-edge alignment stable by reserving the chevron slot even for non-expandable rows
- Do not reserve a fixed duration column when the duration is empty

#### Collapsible Agent Activities
Activities are collapsed by default, showing a one-line summary. The card itself is the toggle target (no extra expand button).

## Full-Screen Overlays

Settings and other full-screen views use:

```tsx
<div className="fixed inset-0 z-50 flex" style={{ backgroundColor: '#f5f5f5' }}>
  {/* Sidebar */}
  <div style={{ width: '244px' }}>...</div>

  {/* Content Panel */}
  <div style={{
    margin: '8px 8px 8px 0',
    backgroundColor: '#fcfcfc',
    borderRadius: '4px',
    boxShadow: '...'
  }}>...</div>
</div>
```

## Best Practices

1. **Use inline styles for colors** - Ensures consistent color application without Tailwind class conflicts
2. **Use Tailwind for layout** - Flexbox, spacing, sizing
3. **Extract shared components** - Place in `shared/` folder
4. **Consistent header heights** - 39px for content, 52px for sidebar top
5. **Subtle shadows** - Use the standard box-shadow for floating panels
6. **Smooth transitions** - Add `transition-colors` for interactive elements
