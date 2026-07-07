<script setup lang='ts'>
interface Skill {
  id: string
  name: string
  description: string
  enabled: boolean
}

const props = defineProps<{
  skills: Skill[]
  activeSkillId: string | null
}>()

const emit = defineEmits<{
  (e: 'select', skillId: string | null): void
}>()
</script>

<template>
  <div class="skill-chips" v-if="skills.length">
    <a-button
      size="mini"
      :type="!activeSkillId ? 'primary' : 'default'"
      @click="$emit('select', null)"
    >全部</a-button>
    <a-button
      v-for="skill in skills.filter(s => s.enabled)"
      :key="skill.id"
      size="mini"
      :type="activeSkillId === skill.id ? 'primary' : 'default'"
      :title="skill.description"
      @click="$emit('select', skill.id)"
    >{{ skill.name }}</a-button>
  </div>
</template>

<style scoped>
.skill-chips {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  padding: 4px 0;
}
</style>
