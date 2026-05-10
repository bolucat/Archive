<template>
  <div class="modal-card" style="max-width: 560px; margin: auto">
    <header class="modal-card-head">
      <p class="modal-card-title">{{ $t("customInbound.title") }}</p>
    </header>
    <section class="modal-card-body">
      <!-- Existing custom inbounds list -->
      <b-table
        :data="inbounds"
        :mobile-cards="false"
        bordered
        narrowed
        style="margin-bottom: 1rem"
      >
        <b-table-column v-slot="props" :label="$t('customInbound.tag')" width="160">
          <code>{{ props.row.tag }}</code>
        </b-table-column>
        <b-table-column v-slot="props" :label="$t('customInbound.protocol')" width="80">
          <b-tag :type="props.row.protocol === 'socks' ? 'is-info' : 'is-success'" size="is-small">
            {{ props.row.protocol.toUpperCase() }}
          </b-tag>
        </b-table-column>
        <b-table-column v-slot="props" :label="$t('customInbound.port')" width="100">
          {{ props.row.port }}
        </b-table-column>
        <b-table-column v-slot="props" :label="$t('operations.name')" width="80">
          <b-button
            size="is-small"
            type="is-danger"
            icon-left="delete"
            @click="handleDelete(props.row.tag)"
          ></b-button>
        </b-table-column>
        <template #empty>
          <div style="text-align: center; padding: 1rem; color: #888">
            {{ $t("customInbound.empty") }}
          </div>
        </template>
      </b-table>

      <!-- Add new inbound form -->
      <div class="box" style="padding: 0.75rem">
        <p class="is-size-6 has-text-weight-semibold" style="margin-bottom: 0.5rem">
          {{ $t("customInbound.addNew") }}
        </p>
        <b-field grouped>
          <b-field :label="$t('customInbound.tag')" expanded label-position="on-border">
            <b-input
              v-model="form.tag"
              :placeholder="$t('customInbound.tagPlaceholder')"
            ></b-input>
          </b-field>
          <b-field :label="$t('customInbound.protocol')" label-position="on-border">
            <b-select v-model="form.protocol">
              <option value="socks">SOCKS</option>
              <option value="http">HTTP</option>
            </b-select>
          </b-field>
          <b-field :label="$t('customInbound.port')" label-position="on-border">
            <b-input
              v-model.number="form.port"
              type="number"
              min="1"
              max="65535"
              style="width: 100px"
              :placeholder="$t('customInbound.portPlaceholder')"
            ></b-input>
          </b-field>
          <b-field label=" " label-position="on-border">
            <b-button type="is-primary" :loading="adding" @click="handleAdd">
              {{ $t("operations.add") }}
            </b-button>
          </b-field>
        </b-field>
        <b-message type="is-info" size="is-small" class="after-line-dot5">
          {{ $t("customInbound.hint") }}
        </b-message>
      </div>
    </section>
    <footer class="modal-card-foot flex-end">
      <b-button @click="$emit('close')">{{ $t("operations.close") }}</b-button>
    </footer>
  </div>
</template>

<script>
import { handleResponse } from "@/assets/js/utils";
import i18n from "@/plugins/i18n";

export default {
  name: "ModalCustomInbound",
  i18n,
  data: () => ({
    inbounds: [],
    form: {
      tag: "",
      protocol: "socks",
      port: "",
    },
    adding: false,
  }),
  created() {
    this.fetchInbounds();
  },
  methods: {
    fetchInbounds() {
      this.$axios({ url: apiRoot + "/customInbound" }).then((res) => {
        if (res.data.code === "SUCCESS") {
          this.inbounds = res.data.data.inbounds || [];
        }
      });
    },
    handleAdd() {
      if (!this.form.tag || !this.form.port) {
        this.$buefy.toast.open({
          message: this.$t("customInbound.fillAll"),
          type: "is-warning",
          position: "is-top",
          queue: false,
        });
        return;
      }
      this.adding = true;
      this.$axios({
        url: apiRoot + "/customInbound",
        method: "post",
        data: {
          tag: this.form.tag.trim(),
          protocol: this.form.protocol,
          port: Number(this.form.port),
        },
      })
        .then((res) => {
          handleResponse(res, this, () => {
            this.inbounds = res.data.data.inbounds || [];
            this.form = { tag: "", protocol: "socks", port: "" };
          });
        })
        .finally(() => {
          this.adding = false;
        });
    },
    handleDelete(tag) {
      this.$buefy.dialog.confirm({
        message: this.$t("customInbound.deleteConfirm", { tag }),
        type: "is-danger",
        confirmText: this.$t("operations.delete"),
        cancelText: this.$t("operations.cancel"),
        onConfirm: () => {
          this.$axios({
            url: apiRoot + "/customInbound",
            method: "delete",
            data: { tag },
          }).then((res) => {
            handleResponse(res, this, () => {
              this.inbounds = res.data.data.inbounds || [];
            });
          });
        },
      });
    },
  },
};
</script>
