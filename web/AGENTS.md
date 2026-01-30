# Luban Web Frontend

## Project Structure

```
web/
├── app/                    # Next.js app router
│   └── page.tsx           # Entry point, renders LubanIDE
├── components/
│   ├── luban-ide.tsx      # Main IDE component
│   ├── luban-layout.tsx   # Core layout (sidebar + floating panel)
│   ├── luban-sidebar.tsx  # Navigation sidebar
│   ├── inbox-view.tsx     # Inbox with split view
│   ├── task-list-view.tsx # Task list grouped by status
│   ├── task-detail-view.tsx # Full task view with chat
│   ├── chat-panel.tsx     # Chat/conversation component
│   ├── right-sidebar.tsx  # Properties sidebar
│   ├── settings-panel.tsx # Full-screen settings
│   └── shared/
│       └── task-header.tsx # Reusable header component
└── docs/
    └── design-system.md   # Design principles and guidelines
```

## Design Principles

See `docs/design-system.md` for detailed design guidelines.

### Quick Reference

#### Colors
| Purpose | Color |
|---------|-------|
| Page background | `#f5f5f5` |
| Content panel | `#fcfcfc` |
| Border | `#ebebeb` |
| Primary text | `#1b1b1b` |
| Secondary text | `#6b6b6b` |
| Muted text | `#9b9b9b` |
| Active state | `#e8e8e8` |
| Hover state | `#eeeeee` |
| Primary accent | `#5e6ad2` |

#### Layout
- Sidebar width: `244px`
- Header height: `39px` (content), `52px` (sidebar top)
- Content panel: `margin: 8px 8px 8px 0`, `border-radius: 4px`

#### Typography
- Primary text: `13px`
- Secondary text: `12px`
- Badges: `11px`
- Titles: `14px`

## Component Hierarchy

```
LubanIDE
├── LubanLayout
│   ├── LubanSidebar
│   │   ├── Workspace dropdown (Settings)
│   │   ├── NavItem (Inbox)
│   │   └── Section (Favorites, Projects)
│   └── Content Panel
│       ├── InboxView (split: list + preview)
│       ├── TaskListView (grouped list)
│       └── TaskDetailView (chat + sidebar)
└── SettingsPanel (full-screen overlay)
```

## Development Guidelines

1. **Use inline styles for colors** - Use `style={{ color: '#xxx' }}` for consistent color application
2. **Use Tailwind for layout** - Use className for spacing, flexbox, sizing
3. **Shared components** - Extract reusable components to `shared/` folder
4. **Consistent heights** - Headers are 39px or 52px
5. **Dropdown menus** - Use Radix UI `DropdownMenu` component
