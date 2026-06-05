import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface Webhook {
  id: string
  name: string
  url: string
  events: string[]
  custom_headers: Record<string, string>
  max_retries: number
  retry_interval_seconds: number
  timeout_seconds: number
  status: 'active' | 'inactive' | 'failing'
  last_delivered_at: string | null
  last_failure_at: string | null
  consecutive_failures: number
  created_at: string
}

export interface WebhookDelivery {
  id: string
  event_type: string
  payload: Record<string, unknown>
  attempt_number: number
  status: string
  response_status: number | null
  error_message: string | null
  scheduled_at: string
  started_at: string | null
  completed_at: string | null
  request_headers: Record<string, unknown>
  request_body: string | null
  response_headers: Record<string, unknown>
  response_body: string | null
  created_at: string
}

export interface WebhookListResponse {
  data: Webhook[]
}

export interface DeliveryListResponse {
  data: WebhookDelivery[]
}

export interface CreateWebhookRequest {
  name: string
  url: string
  events: string[]
  custom_headers?: Record<string, string>
  max_retries?: number
  retry_interval_seconds?: number
  timeout_seconds?: number
  status?: string
}

export interface CreateWebhookResponse extends Webhook {
  secret: string
}

const WEBHOOKS_QUERY_KEY = ['webhooks']

export function useWebhooks() {
  return useQuery<WebhookListResponse, ApiError>({
    queryKey: WEBHOOKS_QUERY_KEY,
    queryFn: async () => {
      const response = await api.get('v1/webhooks')
      return response.json<WebhookListResponse>()
    },
  })
}

export function useWebhookDeliveries(webhookId: string | null) {
  return useQuery<DeliveryListResponse, ApiError>({
    queryKey: ['webhooks', webhookId, 'deliveries'],
    queryFn: async () => {
      const response = await api.get(`v1/webhooks/${webhookId}/deliveries`)
      return response.json<DeliveryListResponse>()
    },
    enabled: !!webhookId,
  })
}

export function useCreateWebhook() {
  const queryClient = useQueryClient()

  return useMutation<CreateWebhookResponse, ApiError, CreateWebhookRequest>({
    mutationFn: async (data) => {
      const response = await api.post('v1/webhooks', { json: data })
      return response.json<CreateWebhookResponse>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY })
    },
  })
}

export function useUpdateWebhook() {
  const queryClient = useQueryClient()

  return useMutation<Webhook, ApiError, { webhookId: string } & Partial<CreateWebhookRequest>>({
    mutationFn: async ({ webhookId, ...data }) => {
      const response = await api.put(`v1/webhooks/${webhookId}`, { json: data })
      return response.json<Webhook>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY })
    },
  })
}

export function useDeleteWebhook() {
  const queryClient = useQueryClient()

  return useMutation<void, ApiError, string>({
    mutationFn: async (webhookId) => {
      await api.delete(`v1/webhooks/${webhookId}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY })
    },
  })
}

export function useRetryWebhookDelivery() {
  const queryClient = useQueryClient()

  return useMutation<WebhookDelivery, ApiError, { webhookId: string; deliveryId: string }>({
    mutationFn: async ({ webhookId, deliveryId }) => {
      const response = await api.post(`v1/webhooks/${webhookId}/deliveries/${deliveryId}/retry`)
      return response.json<WebhookDelivery>()
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['webhooks', variables.webhookId, 'deliveries'] })
    },
  })
}

export { parseApiError }
