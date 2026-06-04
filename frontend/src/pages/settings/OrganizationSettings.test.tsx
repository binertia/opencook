import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import OrganizationSettings from './OrganizationSettings'

// Mock the auth hook
vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => ({
    user: {
      organizations: [{ org_id: 'org_123', org_name: 'test-org', role: 'admin' }],
    },
  }),
}))

// Mock the API
vi.mock('@/lib/api', () => ({
  api: {
    get: vi.fn(),
    put: vi.fn(),
  },
  parseApiError: vi.fn(),
}))

import { api } from '@/lib/api'

const mockGet = api.get as unknown as ReturnType<typeof vi.fn>

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

describe('OrganizationSettings', () => {
  const mockOrg = {
    id: 'org_123',
    name: 'acme-corp',
    display_name: 'Acme Corporation',
    description: 'Test org',
    settings: {
      default_routing_strategy: 'quality',
      allowed_providers: ['openai'],
      blocked_models: ['gpt-4o-mini'],
      token_budget: {
        monthly_limit: null,
        cost_budget_usd: 1000,
        alert_threshold_percent: 80,
      },
    },
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    created_by: 'user_1',
    status: 'active',
  }

  beforeEach(() => {
    vi.clearAllMocks()
    mockGet.mockResolvedValue({
      json: () => Promise.resolve(mockOrg),
    } as never)
  })

  it('loads and displays organization data', async () => {
    render(<OrganizationSettings />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByLabelText('Organization Name')).toHaveValue('acme-corp')
    })

    expect(screen.getByLabelText('Display Name')).toHaveValue('Acme Corporation')
  })

  it('validates invalid slug with uppercase letters', async () => {
    const user = userEvent.setup()
    render(<OrganizationSettings />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByLabelText('Organization Name')).toHaveValue('acme-corp')
    })

    const slugInput = screen.getByLabelText('Slug')
    await user.clear(slugInput)
    await user.type(slugInput, 'Acme Corp')

    const saveButton = screen.getByRole('button', { name: /save changes/i })
    await user.click(saveButton)

    await waitFor(() => {
      expect(screen.getByText(/only lowercase letters/i)).toBeInTheDocument()
    })
  })

  it('shows loading state initially', () => {
    mockGet.mockImplementation(
      () => new Promise(() => {}) // never resolves
    )
    render(<OrganizationSettings />, { wrapper: Wrapper })
    expect(screen.getByText(/loading organization/i)).toBeInTheDocument()
  })

  it('shows error state on API failure', async () => {
    mockGet.mockRejectedValue(new Error('Network error'))
    render(<OrganizationSettings />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText(/failed to load organization/i)).toBeInTheDocument()
    })
  })
})
