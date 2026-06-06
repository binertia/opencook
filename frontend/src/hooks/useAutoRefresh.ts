import { useEffect, useRef } from 'react'
import { useAuthStore } from '@/store/authStore'
import { useAuth } from './useAuth'

const REFRESH_BUFFER_SEC = 120 // refresh 2 min before expiry

export function useAutoRefresh() {
  const { refresh } = useAuth()
  const expiresAt = useAuthStore((s) => s.expiresAt)
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }

    if (!isAuthenticated || !expiresAt) return

    const nowSec = Math.floor(Date.now() / 1000)
    const delayMs = (expiresAt - nowSec - REFRESH_BUFFER_SEC) * 1000

    if (delayMs <= 0) {
      // Already within buffer or expired — refresh immediately
      refresh()
      return
    }

    timerRef.current = setTimeout(() => {
      refresh()
    }, delayMs)

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
    }
  }, [expiresAt, isAuthenticated, refresh])
}
