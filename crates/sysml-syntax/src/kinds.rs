// SPDX-License-Identifier: Apache-2.0
//! The kinds of tokens and syntax-tree nodes (ADR 0011).
//!
//! Tokens follow the lexical structure of KerML 8.2.2, which SysML shares except for its reserved
//! words (SysML 8.2.2.1.2). Every reserved word of either language has a keyword kind; which words
//! are reserved depends on the language being lexed ([`KERML_RESERVED`], [`SYSML_RESERVED`]).

macro_rules! syntax_kinds {
    (
        tokens { $($(#[$token_meta:meta])* $token:ident),* $(,)? }
        keywords { $($keyword:ident = $keyword_text:literal),* $(,)? }
        symbols { $($symbol:ident = $symbol_text:literal),* $(,)? }
        nodes { $($(#[$node_meta:meta])* $node:ident),* $(,)? }
    ) => {
        /// A token or node kind.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$token_meta])* $token,)*
            $(#[doc = concat!("Keyword `", $keyword_text, "`")] $keyword,)*
            $(#[doc = concat!("Symbol `", $symbol_text, "`")] $symbol,)*
            $($(#[$node_meta])* $node,)*
        }

        impl SyntaxKind {
            /// Every kind, in declaration order (so that `ALL[k as usize] == k`).
            pub const ALL: &'static [SyntaxKind] = &[
                $(SyntaxKind::$token,)* $(SyntaxKind::$keyword,)* $(SyntaxKind::$symbol,)* $(SyntaxKind::$node,)*
            ];

            /// The fixed text of a keyword or symbol.
            pub const fn text(self) -> Option<&'static str> {
                match self {
                    $(SyntaxKind::$keyword => Some($keyword_text),)*
                    $(SyntaxKind::$symbol => Some($symbol_text),)*
                    _ => None,
                }
            }

            /// Whether this is a keyword kind.
            pub const fn is_keyword(self) -> bool {
                matches!(self, $(SyntaxKind::$keyword)|*)
            }

            /// Whether this is a symbol kind (KerML 8.2.2.7).
            pub const fn is_symbol(self) -> bool {
                matches!(self, $(SyntaxKind::$symbol)|*)
            }
        }

        /// The keyword kinds by text, sorted by text.
        pub(crate) const KEYWORDS: &[(&str, SyntaxKind)] = &[$(($keyword_text, SyntaxKind::$keyword),)*];
        /// The symbol kinds by text.
        pub(crate) const SYMBOLS: &[(&str, SyntaxKind)] = &[$(($symbol_text, SyntaxKind::$symbol),)*];
    };
}

syntax_kinds! {
    tokens {
        /// White space and line terminators (KerML 8.2.2.1). Trivia.
        Whitespace,
        /// A byte-order mark (U+FEFF) at the very start of the text (ADR 0013). Trivia.
        ByteOrderMark,
        /// `// …` (KerML 8.2.2.2). Trivia.
        SingleLineNote,
        /// `//* … */` (KerML 8.2.2.2). Trivia.
        MultilineNote,
        /// `/* … */` (KerML 8.2.2.2), which becomes a `Comment` element.
        RegularComment,
        /// BASIC_NAME (KerML 8.2.2.3).
        BasicName,
        /// UNRESTRICTED_NAME, `'…'` (KerML 8.2.2.3).
        UnrestrictedName,
        /// DECIMAL_VALUE (KerML 8.2.2.4).
        DecimalValue,
        /// EXPONENTIAL_VALUE (KerML 8.2.2.4).
        ExponentialValue,
        /// STRING_VALUE, `"…"` (KerML 8.2.2.5).
        StringValue,
        /// Text that is not a valid token; always comes with a diagnostic.
        ErrorToken,
        /// The end of the input, as seen by the parser (never in the tree).
        Eof,
    }
    keywords {
        AboutKw = "about",
        AbstractKw = "abstract",
        AcceptKw = "accept",
        ActionKw = "action",
        ActorKw = "actor",
        AfterKw = "after",
        AliasKw = "alias",
        AllKw = "all",
        AllocateKw = "allocate",
        AllocationKw = "allocation",
        AnalysisKw = "analysis",
        AndKw = "and",
        AsKw = "as",
        AssertKw = "assert",
        AssignKw = "assign",
        AssocKw = "assoc",
        AssumeKw = "assume",
        AtKw = "at",
        AttributeKw = "attribute",
        BehaviorKw = "behavior",
        BindKw = "bind",
        BindingKw = "binding",
        BoolKw = "bool",
        ByKw = "by",
        CalcKw = "calc",
        CaseKw = "case",
        ChainsKw = "chains",
        ClassKw = "class",
        ClassifierKw = "classifier",
        CommentKw = "comment",
        CompositeKw = "composite",
        ConcernKw = "concern",
        ConjugateKw = "conjugate",
        ConjugatesKw = "conjugates",
        ConjugationKw = "conjugation",
        ConnectKw = "connect",
        ConnectionKw = "connection",
        ConnectorKw = "connector",
        ConstKw = "const",
        ConstantKw = "constant",
        ConstraintKw = "constraint",
        CrossesKw = "crosses",
        DatatypeKw = "datatype",
        DecideKw = "decide",
        DefKw = "def",
        DefaultKw = "default",
        DefinedKw = "defined",
        DependencyKw = "dependency",
        DerivedKw = "derived",
        DifferencesKw = "differences",
        DisjoiningKw = "disjoining",
        DisjointKw = "disjoint",
        DoKw = "do",
        DocKw = "doc",
        ElseKw = "else",
        EndKw = "end",
        EntryKw = "entry",
        EnumKw = "enum",
        EventKw = "event",
        ExhibitKw = "exhibit",
        ExitKw = "exit",
        ExposeKw = "expose",
        ExprKw = "expr",
        FalseKw = "false",
        FeatureKw = "feature",
        FeaturedKw = "featured",
        FeaturingKw = "featuring",
        FilterKw = "filter",
        FirstKw = "first",
        FlowKw = "flow",
        ForKw = "for",
        ForkKw = "fork",
        FrameKw = "frame",
        FromKw = "from",
        FunctionKw = "function",
        HastypeKw = "hastype",
        IfKw = "if",
        ImpliesKw = "implies",
        ImportKw = "import",
        InKw = "in",
        IncludeKw = "include",
        IndividualKw = "individual",
        InoutKw = "inout",
        InteractionKw = "interaction",
        InterfaceKw = "interface",
        IntersectsKw = "intersects",
        InvKw = "inv",
        InverseKw = "inverse",
        InvertingKw = "inverting",
        IstypeKw = "istype",
        ItemKw = "item",
        JoinKw = "join",
        LanguageKw = "language",
        LibraryKw = "library",
        LocaleKw = "locale",
        LoopKw = "loop",
        MemberKw = "member",
        MergeKw = "merge",
        MessageKw = "message",
        MetaKw = "meta",
        MetaclassKw = "metaclass",
        MetadataKw = "metadata",
        MultiplicityKw = "multiplicity",
        NamespaceKw = "namespace",
        NonuniqueKw = "nonunique",
        NotKw = "not",
        NullKw = "null",
        ObjectiveKw = "objective",
        OccurrenceKw = "occurrence",
        OfKw = "of",
        OrKw = "or",
        OrderedKw = "ordered",
        OutKw = "out",
        PackageKw = "package",
        ParallelKw = "parallel",
        PartKw = "part",
        PerformKw = "perform",
        PortKw = "port",
        PortionKw = "portion",
        PredicateKw = "predicate",
        PrivateKw = "private",
        ProtectedKw = "protected",
        PublicKw = "public",
        RedefinesKw = "redefines",
        RedefinitionKw = "redefinition",
        RefKw = "ref",
        ReferencesKw = "references",
        RenderKw = "render",
        RenderingKw = "rendering",
        RepKw = "rep",
        RequireKw = "require",
        RequirementKw = "requirement",
        ReturnKw = "return",
        SatisfyKw = "satisfy",
        SendKw = "send",
        SnapshotKw = "snapshot",
        SpecializationKw = "specialization",
        SpecializesKw = "specializes",
        StakeholderKw = "stakeholder",
        StandardKw = "standard",
        StateKw = "state",
        StepKw = "step",
        StructKw = "struct",
        SubclassifierKw = "subclassifier",
        SubjectKw = "subject",
        SubsetKw = "subset",
        SubsetsKw = "subsets",
        SubtypeKw = "subtype",
        SuccessionKw = "succession",
        TerminateKw = "terminate",
        ThenKw = "then",
        TimesliceKw = "timeslice",
        ToKw = "to",
        TransitionKw = "transition",
        TrueKw = "true",
        TypeKw = "type",
        TypedKw = "typed",
        TypingKw = "typing",
        UnionsKw = "unions",
        UntilKw = "until",
        UseKw = "use",
        VarKw = "var",
        VariantKw = "variant",
        VariationKw = "variation",
        VerificationKw = "verification",
        VerifyKw = "verify",
        ViaKw = "via",
        ViewKw = "view",
        ViewpointKw = "viewpoint",
        WhenKw = "when",
        WhileKw = "while",
        XorKw = "xor",
    }
    symbols {
        LParen = "(",
        RParen = ")",
        LBrace = "{",
        RBrace = "}",
        LBracket = "[",
        RBracket = "]",
        Semicolon = ";",
        Comma = ",",
        Tilde = "~",
        At = "@",
        Hash = "#",
        Percent = "%",
        Amp = "&",
        Caret = "^",
        Pipe = "|",
        Star = "*",
        StarStar = "**",
        Plus = "+",
        Minus = "-",
        Slash = "/",
        Arrow = "->",
        Dollar = "$",
        Dot = ".",
        DotDot = "..",
        Colon = ":",
        ColonColon = "::",
        ColonGt = ":>",
        ColonGtGt = ":>>",
        ColonColonGt = "::>",
        FatArrow = "=>",
        Lt = "<",
        LtEq = "<=",
        Eq = "=",
        ColonEq = ":=",
        EqEq = "==",
        EqEqEq = "===",
        NotEq = "!=",
        NotEqEq = "!==",
        Gt = ">",
        GtEq = ">=",
        Question = "?",
        QuestionQuestion = "??",
        DotQuestion = ".?",
    }
    nodes {
        /// The root namespace of a file (KerML 8.2.3.4.1).
        RootNamespace,
        /// Tokens the parser could not place, or a construct it does not parse yet; always comes
        /// with a diagnostic.
        Error,

        // Names and bodies (KerML 8.2.3.1, 8.2.3.4.1, 8.2.4.3.5).
        /// `<short> name`: the declared short name and name of an element (KerML 8.2.3.1).
        Identification,
        /// `<short>` (KerML 8.2.3.1).
        ShortName,
        /// `$::A::B` (KerML 8.2.3.4.1).
        QualifiedName,
        /// `a.b.c`: a feature chain, each qualified name a feature chaining (KerML 8.2.4.3.5).
        FeatureChain,
        /// `;` or `{ … }`: a relationship, namespace, type or package body.
        Body,

        // Root (KerML 8.2.3).
        /// KerML 8.2.3.2.
        Dependency,
        /// An annotated element after `about` (KerML 8.2.3.3.1).
        Annotation,
        /// KerML 8.2.3.3.2.
        Comment,
        /// `doc` (KerML 8.2.3.3.2).
        Documentation,
        /// `rep … language "…"` (KerML 8.2.3.3.3).
        TextualRepresentation,
        /// `namespace` (KerML 8.2.3.4.1).
        Namespace,
        /// `alias … for …` (KerML 8.2.3.4.1).
        AliasMember,
        /// KerML 8.2.3.4.2.
        Import,

        // Core (KerML 8.2.4).
        /// `type` (KerML 8.2.4.1.1).
        Type,
        /// `classifier` (KerML 8.2.4.2.1).
        Classifier,
        /// KerML 8.2.4.3.1.
        Feature,
        /// The cross feature of an end feature (KerML 8.2.4.3.1).
        CrossFeature,
        /// `specialization … subtype` (KerML 8.2.4.1.2).
        Specialization,
        /// `conjugation … conjugate` (KerML 8.2.4.1.3).
        Conjugation,
        /// `disjoining … disjoint` (KerML 8.2.4.1.4).
        Disjoining,
        /// `specialization … subclassifier` (KerML 8.2.4.2.2).
        Subclassification,
        /// `specialization … typing` (KerML 8.2.4.3.2).
        FeatureTyping,
        /// `specialization … subset` (KerML 8.2.4.3.3).
        Subsetting,
        /// `specialization … redefinition` (KerML 8.2.4.3.4).
        Redefinition,
        /// `inverting … inverse` (KerML 8.2.4.3.6).
        FeatureInverting,
        /// `featuring … by` (KerML 8.2.4.3.7).
        TypeFeaturing,
        /// After `specializes` in a type (KerML 8.2.4.1.2).
        OwnedSpecialization,
        /// After `conjugates` (KerML 8.2.4.1.3).
        OwnedConjugation,
        /// After `disjoint from` (KerML 8.2.4.1.4).
        OwnedDisjoining,
        /// After `unions` (KerML 8.2.4.1.5).
        Unioning,
        /// After `intersects` (KerML 8.2.4.1.5).
        Intersecting,
        /// After `differences` (KerML 8.2.4.1.5).
        Differencing,
        /// After `specializes` in a classifier (KerML 8.2.4.2.2).
        OwnedSubclassification,
        /// After `typed by` (KerML 8.2.4.3.2).
        OwnedFeatureTyping,
        /// After `subsets` (KerML 8.2.4.3.3).
        OwnedSubsetting,
        /// After `references` (KerML 8.2.4.3.3).
        OwnedReferenceSubsetting,
        /// After `crosses` (KerML 8.2.4.3.3).
        OwnedCrossSubsetting,
        /// After `redefines` (KerML 8.2.4.3.4).
        OwnedRedefinition,
        /// After `inverse of` (KerML 8.2.4.3.6).
        OwnedFeatureInverting,
        /// After `featured by` (KerML 8.2.4.3.7).
        OwnedTypeFeaturing,

        // Kernel, the parts the Root and Core layers need first (step 1a.3).
        /// `package` (KerML 8.2.5.13).
        Package,
        /// `standard library package` (KerML 8.2.5.13, ADR 0014).
        LibraryPackage,
        /// `#Name`: prefix metadata (KerML 8.2.5.12).
        PrefixMetadata,
        /// `multiplicity … subsets …` (KerML 8.2.5.11).
        MultiplicitySubset,
        /// `multiplicity … [ … ]` (KerML 8.2.5.11).
        MultiplicityRange,
        /// `[lower..upper]` (KerML 8.2.5.11).
        MultiplicityBounds,
        /// `true`, `false` (KerML 8.2.5.8.4).
        LiteralBoolean,
        /// `"…"` (KerML 8.2.5.8.4).
        LiteralString,
        /// `42` (KerML 8.2.5.8.4).
        LiteralInteger,
        /// `4.2`, `.5`, `1e3` (KerML 8.2.5.8.4).
        LiteralReal,
        /// `*` (KerML 8.2.5.8.4).
        LiteralInfinity,
        /// A feature named by a qualified name (KerML 8.2.5.8.3).
        FeatureReferenceExpression,

        // Kernel (KerML 8.2.5): classifiers.
        /// `datatype` (KerML 8.2.5.1).
        DataType,
        /// `class` (KerML 8.2.5.2).
        Class,
        /// `struct` (KerML 8.2.5.3).
        Structure,
        /// `assoc` (KerML 8.2.5.4).
        Association,
        /// `assoc struct` (KerML 8.2.5.4).
        AssociationStructure,
        /// `behavior` (KerML 8.2.5.6.1).
        Behavior,
        /// `function` (KerML 8.2.5.7.1).
        Function,
        /// `predicate` (KerML 8.2.5.7.3).
        Predicate,
        /// `interaction` (KerML 8.2.5.9.1).
        Interaction,
        /// `metaclass` (KerML 8.2.5.12).
        Metaclass,

        // Kernel: features.
        /// `step` (KerML 8.2.5.6.2).
        Step,
        /// `expr` (KerML 8.2.5.7.2).
        Expression,
        /// `bool` (KerML 8.2.5.7.4).
        BooleanExpression,
        /// `inv` (KerML 8.2.5.7.4).
        Invariant,
        /// `connector` (KerML 8.2.5.5.1).
        Connector,
        /// `binding` (KerML 8.2.5.5.2).
        BindingConnector,
        /// `succession` (KerML 8.2.5.5.3).
        Succession,
        /// `flow` (KerML 8.2.5.9.2).
        Flow,
        /// `succession flow` (KerML 8.2.5.9.2).
        SuccessionFlow,
        /// A connector end: `[m] name references feature` (KerML 8.2.5.5.1).
        ConnectorEnd,
        /// A flow end: `feature.flowFeature` (KerML 8.2.5.9.2).
        FlowEnd,
        /// The payload of a flow, after `of` (KerML 8.2.5.9.2).
        PayloadFeature,
        /// The result expression at the end of a function body (KerML 8.2.5.7.1).
        ResultExpressionMember,
        /// `= …`, `:= …` or `default …` (KerML 8.2.5.10).
        FeatureValue,
        /// `@` or `metadata` (KerML 8.2.5.12).
        MetadataFeature,
        /// `filter …;` in a package (KerML 8.2.5.13).
        ElementFilterMember,
        /// `[ … ]` after an import declaration (KerML 8.2.3.4.2).
        FilterPackageMember,

        // Expressions (KerML 8.2.5.8).
        /// `if … ? … else …` (KerML 8.2.5.8.1).
        ConditionalExpression,
        /// A binary operator expression, including the conditional `??`, `or`, `and`, `implies`
        /// (KerML 8.2.5.8.1).
        BinaryExpression,
        /// A unary operator expression: `+ - ~ not` (KerML 8.2.5.8.1).
        UnaryExpression,
        /// `istype hastype @ as`, and the metaclassification operators `@@ meta`
        /// (KerML 8.2.5.8.1).
        ClassificationExpression,
        /// `all T` (KerML 8.2.5.8.1).
        ExtentExpression,
        /// `a.b` (KerML 8.2.5.8.2).
        FeatureChainExpression,
        /// `a.{ … }` (KerML 8.2.5.8.2).
        CollectExpression,
        /// `a.?{ … }` (KerML 8.2.5.8.2).
        SelectExpression,
        /// `a->f(…)`, `a->f { … }`, `a->f g` (KerML 8.2.5.8.2).
        FunctionOperationExpression,
        /// `a[ … ]` (KerML 8.2.5.8.2).
        BracketExpression,
        /// `a#( … )` (KerML 8.2.5.8.2).
        IndexExpression,
        /// `( … )`: one expression in parentheses, or a sequence `(a, b)` (KerML 8.2.5.8.2).
        SequenceExpression,
        /// `null` or `()` (KerML 8.2.5.8.3).
        NullExpression,
        /// `a.metadata` (KerML 8.2.5.8.3).
        MetadataAccessExpression,
        /// `f(…)` (KerML 8.2.5.8.3).
        InvocationExpression,
        /// `new T(…)` (KerML 8.2.5.8.3, ADR 0015).
        ConstructorExpression,
        /// `( … )` of an invocation (KerML 8.2.5.8.3).
        ArgumentList,
        /// `name = value` in an argument list (KerML 8.2.5.8.3).
        NamedArgument,
        /// `{ … }` (KerML 8.2.5.8.3).
        BodyExpression,
    }
}

impl SyntaxKind {
    /// Trivia: white space and notes, which carry no meaning (KerML 8.2.2.1, 8.2.2.2).
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::ByteOrderMark
                | SyntaxKind::SingleLineNote
                | SyntaxKind::MultilineNote
        )
    }

    /// How the kind is named in a message: `` `;` `` for a fixed text, otherwise in words, for
    /// example "a name".
    pub fn description(self) -> String {
        if let Some(text) = self.text() {
            return format!("`{text}`");
        }
        match self {
            SyntaxKind::BasicName | SyntaxKind::UnrestrictedName => "a name",
            SyntaxKind::DecimalValue | SyntaxKind::ExponentialValue => "a number",
            SyntaxKind::StringValue => "a string",
            SyntaxKind::RegularComment => "a comment",
            SyntaxKind::Whitespace => "white space",
            SyntaxKind::ByteOrderMark => "a byte-order mark",
            SyntaxKind::SingleLineNote | SyntaxKind::MultilineNote => "a note",
            SyntaxKind::ErrorToken => "an invalid token",
            SyntaxKind::Eof => "the end of the input",
            SyntaxKind::RootNamespace => "a root namespace",
            _ => "an element",
        }
        .to_owned()
    }

    pub(crate) fn from_raw(raw: u16) -> SyntaxKind {
        SyntaxKind::ALL[raw as usize]
    }
}

/// The reserved words of KerML (KerML 8.2.2.6), sorted.
pub const KERML_RESERVED: &[&str] = &[
    "about",
    "abstract",
    "alias",
    "all",
    "and",
    "as",
    "assoc",
    "behavior",
    "binding",
    "bool",
    "by",
    "chains",
    "class",
    "classifier",
    "comment",
    "composite",
    "conjugate",
    "conjugates",
    "conjugation",
    "connector",
    "const",
    "crosses",
    "datatype",
    "default",
    "dependency",
    "derived",
    "differences",
    "disjoining",
    "disjoint",
    "doc",
    "else",
    "end",
    "expr",
    "false",
    "feature",
    "featured",
    "featuring",
    "filter",
    "first",
    "flow",
    "for",
    "from",
    "function",
    "hastype",
    "if",
    "implies",
    "import",
    "in",
    "inout",
    "interaction",
    "intersects",
    "inv",
    "inverse",
    "inverting",
    "istype",
    "language",
    "library",
    "locale",
    "member",
    "meta",
    "metaclass",
    "metadata",
    "multiplicity",
    "namespace",
    "nonunique",
    "not",
    "null",
    "of",
    "or",
    "ordered",
    "out",
    "package",
    "portion",
    "predicate",
    "private",
    "protected",
    "public",
    "redefines",
    "redefinition",
    "references",
    "rep",
    "return",
    "specialization",
    "specializes",
    "standard",
    "step",
    "struct",
    "subclassifier",
    "subset",
    "subsets",
    "subtype",
    "succession",
    "then",
    "to",
    "true",
    "type",
    "typed",
    "typing",
    "unions",
    "var",
    "xor",
];

/// The reserved words of SysML (SysML 8.2.2.1.2), sorted.
pub const SYSML_RESERVED: &[&str] = &[
    "about",
    "abstract",
    "accept",
    "action",
    "actor",
    "after",
    "alias",
    "all",
    "allocate",
    "allocation",
    "analysis",
    "and",
    "as",
    "assert",
    "assign",
    "assume",
    "at",
    "attribute",
    "bind",
    "binding",
    "by",
    "calc",
    "case",
    "comment",
    "concern",
    "connect",
    "connection",
    "constant",
    "constraint",
    "crosses",
    "decide",
    "def",
    "default",
    "defined",
    "dependency",
    "derived",
    "do",
    "doc",
    "else",
    "end",
    "entry",
    "enum",
    "event",
    "exhibit",
    "exit",
    "expose",
    "false",
    "filter",
    "first",
    "flow",
    "for",
    "fork",
    "frame",
    "from",
    "hastype",
    "if",
    "implies",
    "import",
    "in",
    "include",
    "individual",
    "inout",
    "interface",
    "istype",
    "item",
    "join",
    "language",
    "library",
    "locale",
    "loop",
    "merge",
    "message",
    "meta",
    "metadata",
    "nonunique",
    "not",
    "null",
    "objective",
    "occurrence",
    "of",
    "or",
    "ordered",
    "out",
    "package",
    "parallel",
    "part",
    "perform",
    "port",
    "private",
    "protected",
    "public",
    "redefines",
    "ref",
    "references",
    "render",
    "rendering",
    "rep",
    "require",
    "requirement",
    "return",
    "satisfy",
    "send",
    "snapshot",
    "specializes",
    "stakeholder",
    "standard",
    "state",
    "subject",
    "subsets",
    "succession",
    "terminate",
    "then",
    "timeslice",
    "to",
    "transition",
    "true",
    "until",
    "use",
    "variant",
    "variation",
    "verification",
    "verify",
    "via",
    "view",
    "viewpoint",
    "when",
    "while",
    "xor",
];

/// The language whose text is lexed and parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    /// KerML (`.kerml`).
    KerML,
    /// SysML (`.sysml`).
    SysML,
}

impl Language {
    /// The keyword kind of `word` in this language, if it is reserved.
    pub fn keyword(self, word: &str) -> Option<SyntaxKind> {
        let reserved = match self {
            Language::KerML => KERML_RESERVED,
            Language::SysML => SYSML_RESERVED,
        };
        reserved.binary_search(&word).ok()?;
        KEYWORDS
            .binary_search_by(|(text, _)| text.cmp(&word))
            .ok()
            .map(|i| KEYWORDS[i].1)
    }
}

/// The `rowan` language of KerML and SysML syntax trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sysml {}

impl rowan::Language for Sysml {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from_raw(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn all_is_in_declaration_order() {
        for (i, k) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(*k as usize, i);
            assert_eq!(SyntaxKind::from_raw(i as u16), *k);
        }
    }

    #[test]
    fn tables_are_sorted_for_binary_search() {
        assert!(KEYWORDS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(KERML_RESERVED.windows(2).all(|w| w[0] < w[1]));
        assert!(SYSML_RESERVED.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn every_keyword_kind_is_reserved_in_some_language() {
        assert_eq!(KEYWORDS.len(), 172);
        for (text, _) in KEYWORDS {
            assert!(
                KERML_RESERVED.contains(text) || SYSML_RESERVED.contains(text),
                "{text}"
            );
        }
    }
}
