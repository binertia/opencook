import ky, { HTTPError } from 'ky'
import { useAuthStore } from '@/store/authStore'

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api'

function getCookie(name: string): string | undefined {
  const match = document.cookie.match(new RegExp('(^| )' + name + '=([^;]+)'))
  if (match) return match[2]
}

export const api = ky.create({
  prefixUrl: API_BASE_URL,
  credentials: 'include',
  timeout: 30000,
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
    return {
      message: body.message || body.error || error.message,
      code: body.code,
      status: error.response.status,
    }
  }
  if (error instanceof Error) {
    return { message: error.message, status: 0 }
  }
  return { message: 'Unknown error', status: 0 }
}
