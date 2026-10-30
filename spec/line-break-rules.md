# Line Break Rules

## Purpose

This document defines where a long BSL line should be broken when the
`ПроверкаДлиныСтрок` (LineLength) checker reports it as too long (see the
scenario contract in `spec/parser-strategy.md`). Without this semantics the
diagnostic is not actionable: an author facing a long line needs to know
where the break goes.

The rules are advisory for code authors. They are NOT enforced by any
`prec-bsl` scenario in this repo; a future line-break checker (issue #5,
`IncorrectLineBreak`) is the prospective consumer of this document.

## Canon

The rules were extracted from the real yaxunit migration PR
bia-technologies/yaxunit #708 (merged as commit `79d950ae`), which closed 10
LineLength findings while keeping the code readable. The examples below are
illustrative; consult the yaxunit commit for real-world shape.

## Rules

### 1. `;`, `)`, `);` attach to the previous line

Statement terminators and closing parentheses never stay alone on a line.
They are glued to the last argument/operand of the previous line:

```bsl
Возврат СтрШаблон("%1.%2.НайтиПоКоду(""%3""); // %4",
	ОписаниеОбъектаМетаданных.ОписаниеТипа.ИмяКоллекции,
	ОписаниеОбъектаМетаданных.Имя,
	Значение.Код,
	Значение.Наименование);
```

### 2. Binary operators move to the next line

A binary operator (`И`, `Или`, `+`, and similar) is not left dangling at the
end of a line; it opens the next line:

```bsl
Если ЗначениеЗаполнено(Код)
	И ЗначениеЗаполнено(Наименование) Тогда
	Возврат Истина;
КонецЕсли;
```

### 3. Call arguments: comma stays, argument moves

When breaking a call across lines, the comma stays at the end of the previous
line and the next argument starts a new line indented one tab from the start
of the call:

```bsl
Справочник = ЮТест.Данные().КонструкторОбъекта("Справочник.Товары")
	.Установить("Наименование", НаименованиеТовара)
	.Записать(, Истина);
```

### 4. A long string literal is moved, not cut

A string literal that makes the line too long is moved to its own line as a
whole. It is never split into pieces or broken mid-literal:

```bsl
ОжидаемоеЗначение =
	"| Строка  | Число |        Дата        | Булево |
	||--------|-------|--------------------|--------|
	|| YaxUnit | 3.14 | 01.01.2025 0:00:00 |   Да   |";
```

### 5. Procedure signature: one parameter per line

A long procedure signature puts each parameter on its own indented line:

```bsl
Процедура ЗагрузитьНастройкиТестов(
		Знач ПараметрыТестов,
		Знач ОписанияКоллекций,
		Знач ПараметрыЗагрузки)
	// тело процедуры
КонецПроцедуры
```

### 6. Indentation is tabs, CRLF is preserved

Indentation for continuation lines uses tab characters (one tab per level).
Line endings of the file are preserved: breaking a line never rewrites the
file's CRLF or LF convention.
