use std::collections::HashSet;

use crate::ast::{InvoiceField, InvoiceItem, Span, Type};
use crate::hir::HirProgram;

use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn check_invoice_declaration(
        &self,
        hir: &HirProgram<'_>,
        fields: &[InvoiceField],
        items: &[InvoiceItem],
        span: Span,
        scope: &Scope,
    ) -> CheckResult<()> {
        self.check_invoice_contract(fields, items, span)?;

        for field in fields {
            let actual = self.infer_expr_with_hir(hir, &field.value, scope)?;
            self.check_invoice_field(&field.key, &actual, field.span)?;
        }

        for item in items {
            self.check_invoice_item(hir, item, scope)?;
        }

        Ok(())
    }

    pub(super) fn check_invoice_contract(
        &self,
        fields: &[InvoiceField],
        items: &[InvoiceItem],
        span: Span,
    ) -> CheckResult<()> {
        let mut keys = HashSet::new();
        let mut has_customer = false;
        let mut has_currency = false;
        let mut has_total = false;

        for field in fields {
            if !keys.insert(field.key.as_str()) {
                return Err(self.error(
                    field.span,
                    format!("Invoice field '{}' declarado mais de uma vez", field.key),
                ));
            }
            has_customer |= field.key == "customer";
            has_currency |= field.key == "currency";
            has_total |= field.key == "total";
        }

        if !has_customer {
            return Err(self.error(span, "Invoice deve declarar customer"));
        }
        if !has_currency {
            return Err(self.error(span, "Invoice deve declarar currency"));
        }
        if items.is_empty() && !has_total {
            return Err(self.error(span, "Invoice deve declarar item ou total"));
        }

        Ok(())
    }

    pub(super) fn check_invoice_field(
        &self,
        key: &str,
        actual: &Type,
        span: Span,
    ) -> CheckResult<()> {
        match key {
            "customer" | "service" | "currency" => ensure_assignable(&Type::String, actual)
                .map_err(|e| self.error(span, format!("Invoice field '{}' inválido: {}", key, e))),
            "discount" | "total" => ensure_assignable(&Type::Money, actual)
                .map_err(|e| self.error(span, format!("Invoice field '{}' inválido: {}", key, e))),
            "tax" => {
                if matches!(actual, Type::Int | Type::Float | Type::Unknown) {
                    Ok(())
                } else {
                    Err(self.error(
                        span,
                        format!(
                            "Invoice field 'tax' espera int ou float, encontrado {}",
                            type_name(actual)
                        ),
                    ))
                }
            }
            _ => Ok(()),
        }
    }

    fn check_invoice_item(
        &self,
        hir: &HirProgram<'_>,
        item: &InvoiceItem,
        scope: &Scope,
    ) -> CheckResult<()> {
        let description = self.infer_expr_with_hir(hir, &item.description, scope)?;
        ensure_assignable(&Type::String, &description).map_err(|e| {
            self.error(
                item.span,
                format!("Invoice item description inválida: {}", e),
            )
        })?;

        let qty = self.infer_expr_with_hir(hir, &item.qty, scope)?;
        if !matches!(qty, Type::Int | Type::Float) {
            return Err(self.error(
                item.span,
                format!(
                    "Invoice item qty espera int ou float, encontrado {}",
                    type_name(&qty)
                ),
            ));
        }

        let price = self.infer_expr_with_hir(hir, &item.price, scope)?;
        ensure_assignable(&Type::Money, &price)
            .map_err(|e| self.error(item.span, format!("Invoice item price inválido: {}", e)))
    }
}
