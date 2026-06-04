import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import ProviderDetailPage from './ProviderDetailPage'

const mockApiGet = vi.fn()
const mockApiPut = vi.fn()
const mockApiPost = vi.fn()
const mockApiDelete = vi.fn()

vi.mock('@/lib/api', () => ({
  api: {
    get: (...args: unknown[]) => mockApiGet(...args),
    put: (...args: unknown[]) => mockApiPut(...args),
    post: (...args: unknown[]) => mockApiPost(...args),
    delete: (...args: unknown[]) => mockApiDelete(...args),
  },
  parseApiError: vi.fn((e) => Promise.resolve({ message: String(e), status: 500 })),
}))

function Wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/providers/openai']}>
        <Routes>
          <Route path="/providers/:providerId" element={children} />
          <Route path="/providers" element={<div>Providers List</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

const mockProvider = {
  id: 'openai',
  name: 'OpenAI',
  kind: 'openai',
  base_url: 'https://api.openai.com/v1',
  status: 'active' as const,
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
  routing_weight: 100,
  priority: 50,
  models: [
    {
      id: 'gpt-4o',
      name: 'GPT-4o',
      context_window: 128000,
      capabilities: ['chat', 'vision'],
      pricing: { input_per_1m_tokens: 2.5, output_per_1m_tokens: 10.0, currency: 'USD' },
      status: 'active' as const,
    },
    {
      id: 'gpt-4o-mini',
      name: 'GPT-4o Mini',
      context_window: 128000,
      capabilities: ['chat'],
      pricing: { input_per_1m_tokens: 0.15, output_per_1m_tokens: 0.6, currency: 'USD' },
      status: 'inactive' as const,
    },
  ],
}

const mockHealth = {
  provider_id: 'openai',
  status: 'healthy' as const,
  latency_ms: 350,
  error_rate: 0.001,
  last_checked: '2024-06-04T00:00:00Z',
}

const mockHealthHistory = {
  object: 'list' as const,
  data: [
    { checked_at: '2024-06-03T23:00:00Z', status: 'healthy' as const, latency_ms: 320, error: null },
    { checked_at: '2024-06-03T22:00:00Z', status: 'healthy' as const, latency_ms: 340, error: null },
    { checked_at: '2024-06-03T21:00:00Z', status: 'degraded' as const, latency_ms: 2500, error: 'Latency above threshold' },
  ],
}

const mockRoutingRules = {
  object: 'list' as const,
  data: [] as Array<{ id: string; name: string; enabled: boolean; strategy: { providers: Array<{ provider_id: string; weight: number }> } }>,
}

describe('ProviderDetailPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockApiGet.mockImplementation((url: string) => {
      if (url.includes('/health-history')) {
        return Promise.resolve({ json: () => Promise.resolve(mockHealthHistory) })
      }
      if (url.includes('/health')) {
        return Promise.resolve({ json: () => Promise.resolve(mockHealth) })
      }
      if (url.includes('routing-rules')) {
        return Promise.resolve({ json: () => Promise.resolve(mockRoutingRules) })
      }
      return Promise.resolve({ json: () => Promise.resolve(mockProvider) })
    })
    mockApiPut.mockResolvedValue({ json: () => Promise.resolve(mockProvider) })
    mockApiDelete.mockResolvedValue(undefined)
  })

  it('renders provider info card', async () => {
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument()
    })

    expect(screen.getByText('Provider Info')).toBeInTheDocument()
    expect(screen.getByText('https://api.openai.com/v1')).toBeInTheDocument()
    expect(screen.getByText('100')).toBeInTheDocument()
    expect(screen.getByText('50')).toBeInTheDocument()
  })

  it('renders model list with toggle switches', async () => {
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('GPT-4o')).toBeInTheDocument()
    })

    expect(screen.getByText('Models')).toBeInTheDocument()
    expect(screen.getByText('GPT-4o Mini')).toBeInTheDocument()

    const toggles = screen.getAllByRole('checkbox')
    expect(toggles.length).toBe(2)
    expect(toggles[0]).toBeChecked()
    expect(toggles[1]).not.toBeChecked()
  })

  it('sends PUT request when model toggle is clicked', async () => {
    const user = userEvent.setup()
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('GPT-4o')).toBeInTheDocument()
    })

    const toggle = screen.getAllByRole('checkbox')[0]
    await user.click(toggle)

    await waitFor(() => {
      expect(mockApiPut).toHaveBeenCalled()
    })

    const [url, options] = mockApiPut.mock.calls[0]
    expect(url).toBe('v1/providers/openai')
    const body = (options as { json: unknown }).json
    expect(body).toMatchObject({
      models: expect.arrayContaining([
        expect.objectContaining({ id: 'gpt-4o', enabled: false }),
      ]),
    })
  })

  it('renders health chart section with data', async () => {
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('Health History (Last 24h)')).toBeInTheDocument()
    })

    // In jsdom recharts may not render SVG due to 0-size container,
    // but the section presence and lack of "no data" message confirms data loaded.
    expect(screen.queryByText(/no health data available/i)).not.toBeInTheDocument()
  })

  it('shows recent errors table', async () => {
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('Recent Errors')).toBeInTheDocument()
    })

    expect(screen.getByText('Latency above threshold')).toBeInTheDocument()
  })

  it('shows delete confirmation for unused provider', async () => {
    const user = userEvent.setup()
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument()
    })

    const deleteBtn = screen.getByRole('button', { name: /delete/i })
    await user.click(deleteBtn)

    await waitFor(() => {
      expect(screen.getByText(/are you sure/i)).toBeInTheDocument()
    })

    const confirmBtn = screen.getByRole('button', { name: /^delete$/i })
    await user.click(confirmBtn)

    await waitFor(() => {
      expect(mockApiDelete).toHaveBeenCalledWith('v1/providers/openai')
    })
  })

  it('blocks delete for provider referenced by routing rules', async () => {
    mockApiGet.mockImplementation((url: string) => {
      if (url.includes('/health-history')) {
        return Promise.resolve({ json: () => Promise.resolve(mockHealthHistory) })
      }
      if (url.includes('/health')) {
        return Promise.resolve({ json: () => Promise.resolve(mockHealth) })
      }
      if (url.includes('routing-rules')) {
        return Promise.resolve({
          json: () =>
            Promise.resolve({
              object: 'list',
              data: [
                {
                  id: 'rule_1',
                  name: 'Cost-Optimized Routing',
                  enabled: true,
                  strategy: {
                    providers: [{ provider_id: 'openai', weight: 100 }],
                  },
                },
              ],
            }),
        })
      }
      return Promise.resolve({ json: () => Promise.resolve(mockProvider) })
    })

    const user = userEvent.setup()
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument()
    })

    const deleteBtn = screen.getByRole('button', { name: /delete/i })
    await user.click(deleteBtn)

    await waitFor(() => {
      expect(screen.getByText(/cannot delete provider/i)).toBeInTheDocument()
    })

    expect(screen.getByText('Cost-Optimized Routing')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^delete$/i })).not.toBeInTheDocument()
  })

  it('shows error state when provider fails to load', async () => {
    mockApiGet.mockRejectedValue(new Error('Network error'))
    render(<ProviderDetailPage />, { wrapper: Wrapper })

    await waitFor(() => {
      expect(screen.getByText(/failed to load provider/i)).toBeInTheDocument()
    })

    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument()
  })
})
