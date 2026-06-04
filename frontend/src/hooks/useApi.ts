import {
  useQuery,
  useMutation,
  type UseQueryOptions,
  type UseMutationOptions,
} from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export function useApiQuery<T>(
  key: string[],
  url: string,
  options?: Omit<UseQueryOptions<T, ApiError, T, string[]>, 'queryKey' | 'queryFn'>
) {
  return useQuery<T, ApiError, T, string[]>({
    queryKey: key,
    queryFn: async () => {
      const response = await api.get(url)
      return response.json<T>()
    },
    ...options,
  })
}

export function useApiMutation<T, V = unknown>(
  url: string,
  options?: Omit<UseMutationOptions<T, ApiError, V>, 'mutationFn'>
) {
  return useMutation<T, ApiError, V>({
    mutationFn: async (variables) => {
      const response = await api.post(url, {
        json: variables,
      })
      return response.json<T>()
    },
    ...options,
  })
}

export { parseApiError }
