import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import UsersPage from './UsersPage'

vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => ({
    user: {
      id: 'user_1',
      email: 'owner@example.com',
      name: 'Owner',
      role: 'owner',
      organizations: [{ org_id: 'org_123', org_name: 'test-org', role: 'owner' }],
    },
  }),
}))

const mockApiGet = vi.fn()
vi.mock('@/lib/api', () => ({
  api: {
    get: (...args: unknown[]) => mockApiGet(...args),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
  parseApiError: vi.fn(),
}))

function Wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return (
    <QueryClientProvider client={client}>
      <BrowserRouter>{children}</BrowserRouter>
    </QueryClientProvider>
  )
}

describe('UsersPage', () => {
  const mockUsers = {
    object: 'list',
    data: [
      {
        id: 'user_1',
        email: 'owner@example.com',
        name: 'Owner',
        role: 'owner',
        status: 'active',
        last_login_at: '2024-01-01T00:00:00Z',
        created_at: '2024-01-01T00:00:00Z',
      },
      {
        id: 'user_2',
        email: 'admin@example.com',
        name: 'Admin',
        role: 'admin',
        status: 'active',
        last_login_at: null,
        created_at: '2024-01-02T00:00:00Z',
      },
    ],
    pagination: { limit: 20, offset: 0, total: 2, has_more: false },
  }

  beforeEach(() => {
    vi.clearAllMocks()
    mockApiGet.mockResolvedValue({
      json: () => Promise.resolve(mockUsers),
    } as never)
  })

  it('renders user table with correct columns', async () => {
    render(<UsersPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('Owner')).toBeInTheDocument()
    })

    expect(screen.getByText('Email')).toBeInTheDocument()
    expect(screen.getByText('Role')).toBeInTheDocument()
    expect(screen.getAllByText('Status')[0]).toBeInTheDocument()
    expect(screen.getByText('Last Login')).toBeInTheDocument()
  })

  it('shows loading state initially', () => {
    mockApiGet.mockImplementation(() => new Promise(() => {}))
    render(<UsersPage />, { wrapper: Wrapper })
    expect(screen.getByText(/loading users/i)).toBeInTheDocument()
  })

  it('shows error state on API failure', async () => {
    mockApiGet.mockRejectedValue(new Error('Network error'))
    render(<UsersPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText(/failed to load users/i)).toBeInTheDocument()
    })
  })

  it('filters by search term', async () => {
    const user = userEvent.setup()
    render(<UsersPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('Owner')).toBeInTheDocument()
    })

    const searchInput = screen.getByPlaceholderText(/search by name or email/i)
    await user.type(searchInput, 'admin')

    expect(mockApiGet).toHaveBeenCalledWith(expect.stringContaining('search=admin'))
  })

  it('opens invite modal on button click', async () => {
    const user = userEvent.setup()
    render(<UsersPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('Owner')).toBeInTheDocument()
    })

    const inviteButton = screen.getByRole('button', { name: /invite user/i })
    await user.click(inviteButton)

    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Invite User' })).toBeInTheDocument()
  })
})
