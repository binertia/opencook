import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

export default function Providers() {
  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold tracking-tight">Providers</h1>
      <p className="text-muted-foreground">Manage AI provider configurations.</p>
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle>OpenAI</CardTitle>
            <CardDescription>gpt-4, gpt-4-turbo, gpt-3.5-turbo</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">Status: Healthy</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Anthropic</CardTitle>
            <CardDescription>claude-3-opus, claude-3-sonnet, claude-3-haiku</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">Status: Healthy</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Gemini</CardTitle>
            <CardDescription>gemini-pro, gemini-ultra</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">Status: Healthy</p>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
