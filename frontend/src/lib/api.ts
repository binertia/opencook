import ky, { HTTPError } from 'ky'
import { useAuthStore } from '@/store/authStore'

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api'

function getCookie(name: string): string | undefined {
  const match = document.cookie.match(new RegExp('(^| )' + name + '=([^;]+)'))
  if (match) return match[2]
}

// Plain ky instance without auth hooks for token refresh (avoids recursion)
const plainKy = ky.create({ prefixUrl: API_BASE_URL, credentials: 'include' })

export const api = ky.create({
  prefixUrl: API_BASE_URL,
  credentials: 'include',
  timeout: 30000,
  retry: {
    limit: 1,
    statusCodes: [401, 408, 413, 429, 500, 502, 503, 504],
  },
  hooks: {
    beforeRequest: [
      (request) => {
        request.headers.set('Accept', 'application/json')
        if (request.body) {
          request.headers.set('Content-Type', 'application/json')
        }
        const token = useAuthStore.getState().accessToken
        if (token) {
          request.headers.set('Authorization', `Bearer ${token}`)
        }
        // Double-submit CSRF token for state-changing admin requests
        const csrf = getCookie('csrf_token')
        if (csrf) {
          request.headers.set('X-CSRF-Token', csrf)
        }
      },
    ],
    beforeRetry: [
      async ({ request, response, retryCount }) => {
        if (response?.status === 401 && retryCount === 0) {
          const refreshToken = useAuthStore.getState().refreshToken
          if (!refreshToken) {
            useAuthStore.getState().logout()
            return
          }
          try {
            const data = await plainKy
              .post('v1/auth/refresh', {
                json: { refresh_token: refreshToken },
              })
              .json<{
                access_token: string
                refresh_token: string
                expires_in: number
                user: {
                  id: string
                  email: string
                  name: string
                  role: string
                  permissions: string[]
                  organizations?: { org_id: string; org_name: string; role: string }[]
                }
              }>()

            useAuthStore.getState().login(
              data.user,
              data.access_token,
              data.refresh_token,
              data.expires_in
            )
            request.headers.set('Authorization', `Bearer ${data.access_token}`)
          } catch {
            useAuthStore.getState().logout()
            window.location.href = '/login'
          }
        }
      },
    ],
    beforeError: [
      (error) => {
        const { response } = error
        if (response?.body) {
          error.name = 'ApiError'
        }
        return error
      },
    ],
  },
})

export interface ApiError {
  message: string
  code?: string
  status: number
}

export async function parseApiError(error: unknown): Promise<ApiError> {
  if (error instanceof HTTPError) {
    const body = await error.response.json().catch(() => ({}))
    // Backend returns nested error: { error: { code, message, type, ... } }
    const nested = body.error && typeof body.error === 'object' ? body.error : null
    return {
      message:
        body.message ||
        nested?.message ||
        (typeof body.error === 'string' ? body.error : error.message),
      code: body.code || nested?.code,
      status: error.response.status,
    }
  }
  if (error instanceof Error) {
    return { message: error.message, status: 0 }
  }
  return { message: 'Unknown error', status: 0 }
}
