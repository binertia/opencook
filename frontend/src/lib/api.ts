import ky, { HTTPError } from 'ky'
import { useAuthStore } from '@/store/authStore'

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api'
const LOGIN_PATH = `${import.meta.env.BASE_URL}login`.replace(/\/+$/, '')

export const api = ky.create({
  prefixUrl: API_BASE_URL,
  credentials: 'include',
  timeout: 30000,
  hooks: {
    beforeRequest: [
      (request) => {
        request.headers.set('Accept', 'application/json')
        request.headers.set('Content-Type', 'application/json')
        const token = useAuthStore.getState().accessToken
        if (token) {
          request.headers.set('Authorization', `Bearer ${token}`)
        }
      },
    ],
    afterResponse: [
      async (_request, _options, response) => {
        if (response.status === 401) {
          const currentPath = window.location.pathname
          // Avoid infinite redirect loop when already on the login page
          if (currentPath === LOGIN_PATH) {
            return response
          }
          const returnUrl = encodeURIComponent(currentPath + window.location.search)
          window.location.href = `${LOGIN_PATH}?returnUrl=${returnUrl}`
        }
        return response
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
