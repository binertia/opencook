import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useOrganization, useUpdateOrganization } from '@/hooks/useOrganization'
import { useAuth } from '@/hooks/useAuth'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { MultiSelect } from '@/components/ui/multi-select'

const orgSchema = z.object({
  name: z.string().min(1, 'Name is required').max(128, 'Max 128 characters'),
  display_name: z.string().max(256, 'Max 256 characters').optional(),
  description: z.string().max(1024, 'Max 1024 characters').optional(),
  slug: z
    .string()
    .min(1, 'Slug is required')
    .max(128, 'Max 128 characters')
    .regex(/^[a-z0-9-]+$/, 'Only lowercase letters, numbers, and hyphens'),
  billing_email: z.string().email('Invalid email').optional().or(z.literal('')),
  default_routing_strategy: z.enum(['cost', 'latency', 'quality', 'fallback']),
  allowed_providers: z.array(z.string()),
  blocked_models: z.array(z.string()),
})

type OrgForm = z.infer<typeof orgSchema>

const ROUTING_STRATEGIES = [
  { value: 'quality', label: 'Quality' },
  { value: 'cost', label: 'Cost' },
  { value: 'latency', label: 'Latency' },
  { value: 'fallback', label: 'Fallback' },
] as const

const PROVIDER_OPTIONS = [
  { value: 'openai', label: 'OpenAI' },
  { value: 'anthropic', label: 'Anthropic' },
  { value: 'gemini', label: 'Gemini' },
  { value: 'ollama', label: 'Ollama' },
]

const MODEL_OPTIONS = [
  { value: 'gpt-4o', label: 'GPT-4o' },
  { value: 'gpt-4o-mini', label: 'GPT-4o Mini' },
  { value: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
  { value: 'claude-3-opus', label: 'Claude 3 Opus' },
  { value: 'claude-3-sonnet', label: 'Claude 3 Sonnet' },
  { value: 'claude-3-haiku', label: 'Claude 3 Haiku' },
  { value: 'gemini-pro', label: 'Gemini Pro' },
  { value: 'gemini-ultra', label: 'Gemini Ultra' },
]

export default function OrganizationSettings() {
  const { user } = useAuth()
  const orgId = user?.organizations?.[0]?.org_id

  const { data: org, isLoading, error } = useOrganization(orgId)
  const updateOrg = useUpdateOrganization(orgId)

  const {
    register,
    handleSubmit,
    reset,
    setValue,
    watch,
    formState: { errors, isDirty },
  } = useForm<OrgForm>({
    resolver: zodResolver(orgSchema),
    defaultValues: {
      name: '',
      display_name: '',
      description: '',
      slug: '',
      billing_email: '',
      default_routing_strategy: 'quality',
      allowed_providers: [],
      blocked_models: [],
    },
  })

  useEffect(() => {
    if (org) {
      reset({
        name: org.name,
        display_name: org.display_name || '',
        description: org.description || '',
        slug: org.name.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
        billing_email: '',
        default_routing_strategy: org.settings?.default_routing_strategy || 'quality',
        allowed_providers: org.settings?.allowed_providers || [],
        blocked_models: org.settings?.blocked_models || [],
      })
    }
  }, [org, reset])

  const onSubmit = async (data: OrgForm) => {
    await updateOrg.mutateAsync({
      name: data.name,
      display_name: data.display_name,
      description: data.description,
      settings: {
        default_routing_strategy: data.default_routing_strategy,
        allowed_providers: data.allowed_providers,
        blocked_models: data.blocked_models,
      },
    })
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <p className="text-muted-foreground">Loading organization...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
        Failed to load organization: {error.message}
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div>
        <h2 className="text-2xl font-bold tracking-tight">Organization Settings</h2>
        <p className="text-muted-foreground">Manage your organization configuration.</p>
      </div>

      {updateOrg.isSuccess && (
        <div className="rounded-md border border-green-500/50 bg-green-500/10 p-4 text-green-700 dark:text-green-300">
          Settings saved successfully.
        </div>
      )}

      {updateOrg.isError && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          Failed to save: {updateOrg.error?.message || 'Unknown error'}
        </div>
      )}

      <form onSubmit={handleSubmit(onSubmit)} className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>General</CardTitle>
            <CardDescription>Basic organization information.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="name">Organization Name</Label>
              <Input id="name" {...register('name')} />
              {errors.name && (
                <p className="text-sm text-destructive">{errors.name.message}</p>
              )}
            </div>

            <div className="space-y-2">
              <Label htmlFor="display_name">Display Name</Label>
              <Input id="display_name" {...register('display_name')} />
              {errors.display_name && (
                <p className="text-sm text-destructive">{errors.display_name.message}</p>
              )}
            </div>

            <div className="space-y-2">
              <Label htmlFor="slug">Slug</Label>
              <Input id="slug" {...register('slug')} />
              {errors.slug && (
                <p className="text-sm text-destructive">{errors.slug.message}</p>
              )}
            </div>

            <div className="space-y-2">
              <Label htmlFor="billing_email">Billing Email</Label>
              <Input id="billing_email" type="email" {...register('billing_email')} />
              {errors.billing_email && (
                <p className="text-sm text-destructive">{errors.billing_email.message}</p>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Gateway Configuration</CardTitle>
            <CardDescription>Configure routing and model access.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="default_routing_strategy">Default Routing Strategy</Label>
              <Select
                value={watch('default_routing_strategy')}
                onValueChange={(value) =>
                  setValue('default_routing_strategy', value as OrgForm['default_routing_strategy'], {
                    shouldDirty: true,
                  })
                }
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select strategy" />
                </SelectTrigger>
                <SelectContent>
                  {ROUTING_STRATEGIES.map((s) => (
                    <SelectItem key={s.value} value={s.value}>
                      {s.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label>Allowed Providers</Label>
              <MultiSelect
                options={PROVIDER_OPTIONS}
                value={watch('allowed_providers')}
                onChange={(value) =>
                  setValue('allowed_providers', value, { shouldDirty: true })
                }
                placeholder="Select allowed providers..."
              />
            </div>

            <div className="space-y-2">
              <Label>Blocked Models</Label>
              <MultiSelect
                options={MODEL_OPTIONS}
                value={watch('blocked_models')}
                onChange={(value) =>
                  setValue('blocked_models', value, { shouldDirty: true })
                }
                placeholder="Select blocked models..."
              />
            </div>
          </CardContent>
        </Card>

        <div className="flex items-center gap-4">
          <Button type="submit" disabled={!isDirty || updateOrg.isPending}>
            {updateOrg.isPending ? 'Saving...' : 'Save Changes'}
          </Button>
          {isDirty && (
            <Button type="button" variant="ghost" onClick={() => reset()}>
              Reset
            </Button>
          )}
        </div>
      </form>
    </div>
  )
}
