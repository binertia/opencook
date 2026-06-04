import { render, screen, within } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import { Sidebar } from './Sidebar'

describe('Sidebar', () => {
  function Wrapper({ children }: { children: React.ReactNode }) {
    return <BrowserRouter>{children}</BrowserRouter>
  }

  it('renders all navigation links in desktop sidebar', () => {
    render(
      <Sidebar
        open={false}
        onClose={() => {}}
        collapsed={false}
        onToggleCollapse={() => {}}
      />,
      { wrapper: Wrapper }
    )

    const desktop = screen.getByTestId('desktop-sidebar')
    expect(within(desktop).getByText('Dashboard')).toBeInTheDocument()
    expect(within(desktop).getByText('Providers')).toBeInTheDocument()
    expect(within(desktop).getByText('API Keys')).toBeInTheDocument()
    expect(within(desktop).getByText('Analytics')).toBeInTheDocument()
    expect(within(desktop).getByText('Settings')).toBeInTheDocument()
  })

  it('renders section titles when expanded', () => {
    render(
      <Sidebar
        open={false}
        onClose={() => {}}
        collapsed={false}
        onToggleCollapse={() => {}}
      />,
      { wrapper: Wrapper }
    )

    const desktop = screen.getByTestId('desktop-sidebar')
    expect(within(desktop).getByText('Manage')).toBeInTheDocument()
    expect(within(desktop).getByText('Monitor')).toBeInTheDocument()
    expect(within(desktop).getByText('Configure')).toBeInTheDocument()
  })

  it('hides section titles when collapsed', () => {
    render(
      <Sidebar
        open={false}
        onClose={() => {}}
        collapsed={true}
        onToggleCollapse={() => {}}
      />,
      { wrapper: Wrapper }
    )

    const desktop = screen.getByTestId('desktop-sidebar')
    expect(within(desktop).queryByText('Manage')).not.toBeInTheDocument()
    expect(within(desktop).queryByText('Monitor')).not.toBeInTheDocument()
    expect(within(desktop).queryByText('Configure')).not.toBeInTheDocument()
  })
})
