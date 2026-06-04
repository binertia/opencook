import { useState } from 'react'
import { NavLink, useLocation } from 'react-router-dom'
import {
  BarChart3,
  ChevronLeft,
  ChevronRight,
  FileText,
  KeyRound,
  LayoutDashboard,
  Plug,
  Settings,
  Users,
  X,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'

interface NavItem {
  label: string
  href: string
  icon: React.ElementType
}

const manageItems: NavItem[] = [
  { label: 'Providers', href: '/providers', icon: Plug },
  { label: 'API Keys', href: '/keys', icon: KeyRound },
  { label: 'Users', href: '/users', icon: Users },
]

const monitorItems: NavItem[] = [
  { label: 'Analytics', href: '/analytics', icon: BarChart3 },
  { label: 'Request Logs', href: '/requests', icon: FileText },
]

const configureItems: NavItem[] = [
  { label: 'Settings', href: '/settings', icon: Settings },
]

interface SidebarProps {
  open: boolean
  onClose: () => void
  collapsed: boolean
  onToggleCollapse: () => void
}

function NavSection({
  title,
  items,
  collapsed,
}: {
  title: string
  items: NavItem[]
  collapsed: boolean
}) {
  const location = useLocation()

  return (
    <div className="px-3 py-2">
      {!collapsed && (
        <h3 className="mb-2 px-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {title}
        </h3>
      )}
      <nav className="space-y-1">
        {items.map((item) => {
          const isActive = location.pathname === item.href
          return (
            <NavLink
              key={item.href}
              to={item.href}
              className={cn(
                'flex items-center gap-3 rounded-md px-2 py-2 text-sm font-medium transition-colors',
                isActive
                  ? 'bg-primary text-primary-foreground'
                  : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                collapsed && 'justify-center'
              )}
              title={collapsed ? item.label : undefined}
            >
              <item.icon className="h-5 w-5 shrink-0" />
              {!collapsed && <span>{item.label}</span>}
            </NavLink>
          )
        })}
      </nav>
    </div>
  )
}

export function Sidebar({
  open,
  onClose,
  collapsed,
  onToggleCollapse,
}: SidebarProps) {
  void open
  void onClose
  const [mobileOpen, setMobileOpen] = useState(false)

  return (
    <>
      {/* Mobile overlay */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={() => setMobileOpen(false)}
          aria-hidden="true"
        />
      )}

      {/* Mobile sidebar drawer */}
      <aside
        className={cn(
          'fixed inset-y-0 left-0 z-50 w-64 transform border-r bg-background transition-transform duration-200 ease-in-out lg:hidden',
          mobileOpen ? 'translate-x-0' : '-translate-x-full'
        )}
      >
        <div className="flex h-16 items-center justify-between border-b px-4">
          <span className="text-lg font-semibold">AI Gateway</span>
          <Button variant="ghost" size="icon" onClick={() => setMobileOpen(false)}>
            <X className="h-5 w-5" />
          </Button>
        </div>
        <div className="space-y-1 py-4">
          <div className="px-3 py-2">
            <nav className="space-y-1">
              <NavLink
                to="/"
                className={({ isActive }) =>
                  cn(
                    'flex items-center gap-3 rounded-md px-2 py-2 text-sm font-medium transition-colors',
                    isActive
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                  )
                }
                onClick={() => setMobileOpen(false)}
              >
                <LayoutDashboard className="h-5 w-5 shrink-0" />
                <span>Dashboard</span>
              </NavLink>
            </nav>
          </div>
          <NavSection title="Manage" items={manageItems} collapsed={false} />
          <NavSection title="Monitor" items={monitorItems} collapsed={false} />
          <NavSection title="Configure" items={configureItems} collapsed={false} />
        </div>
      </aside>

      {/* Desktop sidebar */}
      <aside
        data-testid="desktop-sidebar"
        className={cn(
          'hidden h-screen flex-col border-r bg-background transition-all duration-200 ease-in-out lg:flex',
          collapsed ? 'w-16' : 'w-64'
        )}
      >
        <div className="flex h-16 items-center justify-between border-b px-4">
          {!collapsed && <span className="text-lg font-semibold">AI Gateway</span>}
          <Button variant="ghost" size="icon" onClick={onToggleCollapse} className="shrink-0">
            {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
          </Button>
        </div>
        <div className="flex-1 space-y-1 overflow-auto py-4">
          <div className="px-3 py-2">
            <nav className="space-y-1">
              <NavLink
                to="/"
                className={({ isActive }) =>
                  cn(
                    'flex items-center gap-3 rounded-md px-2 py-2 text-sm font-medium transition-colors',
                    isActive
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                    collapsed && 'justify-center'
                  )
                }
                title={collapsed ? 'Dashboard' : undefined}
              >
                <LayoutDashboard className="h-5 w-5 shrink-0" />
                {!collapsed && <span>Dashboard</span>}
              </NavLink>
            </nav>
          </div>
          <NavSection title="Manage" items={manageItems} collapsed={collapsed} />
          <NavSection title="Monitor" items={monitorItems} collapsed={collapsed} />
          <NavSection title="Configure" items={configureItems} collapsed={collapsed} />
        </div>
      </aside>
    </>
  )
}
