import { render, screen } from '@testing-library/react'
import { HealthIndicator } from './HealthIndicator'
import type { ProviderHealth } from '@/hooks/useProviders'

describe('HealthIndicator', () => {
  it('renders green for healthy status', () => {
    const health: ProviderHealth = {
      provider_id: 'p1',
      status: 'healthy',
      latency_ms: 120,
      error_rate: 0.001,
      last_checked: '2024-01-01T00:00:00Z',
    }
    render(<HealthIndicator health={health} />)
    expect(screen.getByText('Healthy')).toBeInTheDocument()
  })

  it('renders yellow for degraded status', () => {
    const health: ProviderHealth = {
      provider_id: 'p1',
      status: 'degraded',
      latency_ms: 1500,
      error_rate: 0.03,
      last_checked: '2024-01-01T00:00:00Z',
    }
    render(<HealthIndicator health={health} />)
    expect(screen.getByText('Degraded')).toBeInTheDocument()
  })

  it('renders red for unhealthy status', () => {
    const health: ProviderHealth = {
      provider_id: 'p1',
      status: 'unhealthy',
      latency_ms: 5000,
      error_rate: 0.1,
      last_checked: '2024-01-01T00:00:00Z',
    }
    render(<HealthIndicator health={health} />)
    expect(screen.getByText('Unhealthy')).toBeInTheDocument()
  })

  it('renders gray for unknown status', () => {
    render(<HealthIndicator />)
    expect(screen.getByText('Unknown')).toBeInTheDocument()
  })
})
