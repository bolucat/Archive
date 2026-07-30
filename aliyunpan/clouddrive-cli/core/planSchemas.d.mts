export interface PlanSchemaField {
  name: string
  type: string
  required: boolean
  description: string
}

export interface PlanSchema {
  name: string
  command: string
  operation: string
  destructive: boolean
  undoable: boolean
  requiresDryRun: boolean
  applyRequiresRationale: boolean
  description: string
  fields: PlanSchemaField[]
  itemFields: PlanSchemaField[]
  example: Record<string, unknown>
}

export const PLAN_SCHEMA_VERSION: number
export const PLAN_SCHEMAS: PlanSchema[]
export function listPlanSchemas(options?: { name?: string }): PlanSchema[]
