import { render, screen, waitFor } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Dashboard from './Dashboard'

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

describe('Dashboard', () => {
  const mockData = {
    total_requests: 1250,
    total_cost_usd: 12.34,
    cache_hit_rate: 15.5,
    avg_latency_ms: 245,
    requests_change: 12.5,
    cost_change: -3.2,
    cache_change: 5.1,
    latency_change: -8.4,
    recent_requests: [
      {
        id: 'req_1',
        timestamp: '2024-01-01T12:00:00Z',
        model: 'gpt-4o',
        provider: 'openai',
        status: 'success' as const,
        tokens: 1500,
        cost_usd: 0.045,
        latency_ms: 320,
      },
    ],
    active_providers: [
      { id: 'openai', name: 'OpenAI', status: 'healthy' as const, last_check: '2024-01-01T12:00:00Z' },
      { id: 'anthropic', name: 'Anthropic', status: 'degraded' as const, last_check: '2024-01-01T12:00:00Z' },
    ],
  }

  beforeEach(() => {
    vi.clearAllMocks()
    mockApiGet.mockResolvedValue({
      json: () => Promise.resolve(mockData),
    } as never)
  })

  it('renders KPI cards with correct data', async () => {
    render(<Dashboard />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('1,250')).toBeInTheDocument()
    })

    expect(screen.getByText('$12.34')).toBeInTheDocument()
    expect(screen.getByText('15.5%')).toBeInTheDocument()
    expect(screen.getByText('245ms')).toBeInTheDocument()
  })

  it('shows loading skeletons initially', () => {
    mockApiGet.mockImplementation(() => new Promise(() => {}))
    render(<Dashboard />, { wrapper: Wrapper })

    const skeletons = document.querySelectorAll('.animate-pulse')
    expect(skeletons.length).toBeGreaterThan(0)
  })

  it('shows error state with retry button on API failure', async () => {
    mockApiGet.mockRejectedValue(new Error('Network error'))
    render(<Dashboard />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText(/failed to load dashboard/i)).toBeInTheDocument()
    })

    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument()
  })

  it('calls API with correct time range', async () => {
    render(<Dashboard />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('1,250')).toBeInTheDocument()
    })

    expect(mockApiGet).toHaveBeenCalledWith(expect.stringContaining('range=today'))
  })

  it('renders recent requests table', async () => {
    render(<Dashboard />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('gpt-4o')).toBeInTheDocument()
    })

    expect(screen.getByText('Recent Requests')).toBeInTheDocument()
  })

  it('renders active providers with health status', async () => {
    render(<Dashboard />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument()
    })

    expect(screen.getByText('Anthropic')).toBeInTheDocument()
    expect(screen.getAllByText('healthy')[0]).toBeInTheDocument()
    expect(screen.getByText('degraded')).toBeInTheDocument()
  })
})
