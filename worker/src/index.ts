// hwpx-policy-hub: Cloudflare Worker + D1
// Collects PII-free recognition policies from users, serves aggregated policies.
// No auth required — all data is structural metadata only.

interface Env {
  DB: D1Database;
  CORS_ORIGIN: string;
}

function cors(env: Env): Record<string, string> {
  return {
    'Access-Control-Allow-Origin': env.CORS_ORIGIN || '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  };
}

function json(data: unknown, status = 200, env?: Env): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json', ...(env ? cors(env) : {}) },
  });
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);

    // CORS preflight
    if (req.method === 'OPTIONS') {
      return new Response(null, { status: 204, headers: cors(env) });
    }

    // POST /api/contribute — submit policy feedback
    if (req.method === 'POST' && url.pathname === '/api/contribute') {
      return handleContribute(req, env);
    }

    // GET /api/policies — fetch aggregated policies for wizard startup
    if (req.method === 'GET' && url.pathname === '/api/policies') {
      return handleGetPolicies(env);
    }

    // GET /api/stats — public stats
    if (req.method === 'GET' && url.pathname === '/api/stats') {
      return handleStats(env);
    }

    return json({ error: 'Not found' }, 404, env);
  },
};

async function handleContribute(req: Request, env: Env): Promise<Response> {
  let body: any;
  try {
    body = await req.json();
  } catch {
    return json({ error: 'Invalid JSON' }, 400, env);
  }

  if (!body.forms && !body.fields && !body.policy) {
    return json({ error: 'No policy data provided' }, 400, env);
  }

  // Rate limit: simple per-contributor hash (no auth needed)
  const contributorHash = body.contributorHash || hashString(req.headers.get('CF-Connecting-IP') || 'unknown');

  // Check rate: max 10 contributions per hour per contributor
  const recentCount = await env.DB.prepare(
    "SELECT COUNT(*) as cnt FROM contributions WHERE contributor_hash = ? AND created_at > datetime('now', '-1 hour')"
  ).bind(contributorHash).first<{ cnt: number }>();

  if (recentCount && recentCount.cnt >= 10) {
    return json({ error: 'Rate limited. Try again later.' }, 429, env);
  }

  // Store raw contribution
  await env.DB.prepare(
    'INSERT INTO contributions (contributor_hash, payload_json, forms_count, fields_count) VALUES (?, ?, ?, ?)'
  ).bind(
    contributorHash,
    JSON.stringify(body),
    body.forms?.length || 0,
    body.fields?.length || 0,
  ).run();

  // Merge into aggregated tables
  let formsAdded = 0;
  let fieldsUpdated = 0;

  // Merge forms
  for (const form of (body.forms || [])) {
    const existing = await env.DB.prepare(
      'SELECT fingerprint FROM aggregated_forms WHERE fingerprint = ?'
    ).bind(form.fingerprint).first();

    if (!existing) {
      await env.DB.prepare(
        'INSERT INTO aggregated_forms (fingerprint, row_count, col_count, header_tokens, contributor_count) VALUES (?, ?, ?, ?, 1)'
      ).bind(form.fingerprint, form.rowCount, form.colCount, JSON.stringify(form.headerTokens || [])).run();
      formsAdded++;
    } else {
      await env.DB.prepare(
        "UPDATE aggregated_forms SET contributor_count = contributor_count + 1, updated_at = datetime('now') WHERE fingerprint = ?"
      ).bind(form.fingerprint).run();
    }
  }

  // Merge fields (confidence = weighted average with vote count)
  for (const f of (body.fields || [])) {
    const existing = await env.DB.prepare(
      'SELECT confidence, vote_count FROM aggregated_fields WHERE form_fingerprint = ? AND cell_addr = ?'
    ).bind(f.formFingerprint, f.cellAddr).first<{ confidence: number; vote_count: number }>();

    if (!existing) {
      await env.DB.prepare(
        'INSERT INTO aggregated_fields (form_fingerprint, cell_addr, label_text, label_hash, canonical_field, cell_role, confidence, vote_count) VALUES (?, ?, ?, ?, ?, ?, ?, 1)'
      ).bind(f.formFingerprint, f.cellAddr, f.labelText, f.labelHash, f.canonicalField, f.cellRole || 'label', f.confidence || 0.5).run();
      fieldsUpdated++;
    } else {
      // Weighted running average
      const newVotes = existing.vote_count + 1;
      const newConf = (existing.confidence * existing.vote_count + (f.confidence || 0.5)) / newVotes;
      await env.DB.prepare(
        "UPDATE aggregated_fields SET confidence = ?, vote_count = ?, canonical_field = COALESCE(?, canonical_field), updated_at = datetime('now') WHERE form_fingerprint = ? AND cell_addr = ?"
      ).bind(newConf, newVotes, f.canonicalField, f.formFingerprint, f.cellAddr).run();
      fieldsUpdated++;
    }
  }

  // Merge policy biases
  if (body.policy) {
    for (const biasKey of ['tableKindBiases', 'rowKindBiases', 'cellRoleBiases']) {
      if (!body.policy[biasKey]) continue;
      for (const [family, weights] of Object.entries(body.policy[biasKey])) {
        const existing = await env.DB.prepare(
          'SELECT biases_json, contributor_count FROM aggregated_policies WHERE family = ?'
        ).bind(family).first<{ biases_json: string; contributor_count: number }>();

        if (!existing) {
          await env.DB.prepare(
            'INSERT INTO aggregated_policies (family, biases_json, contributor_count) VALUES (?, ?, 1)'
          ).bind(family, JSON.stringify({ [biasKey]: { [family]: weights } })).run();
        } else {
          // Merge: keep existing biases, add new families
          const merged = JSON.parse(existing.biases_json);
          if (!merged[biasKey]) merged[biasKey] = {};
          merged[biasKey][family] = weights; // latest wins for same family
          await env.DB.prepare(
            "UPDATE aggregated_policies SET biases_json = ?, contributor_count = contributor_count + 1, updated_at = datetime('now') WHERE family = ?"
          ).bind(JSON.stringify(merged), family).run();
        }
      }
    }
  }

  return json({
    ok: true,
    formsAdded,
    fieldsUpdated,
    message: `감사합니다! 양식 ${formsAdded}개, 필드 ${fieldsUpdated}개가 커뮤니티에 공유되었습니다.`,
  }, 200, env);
}

async function handleGetPolicies(env: Env): Promise<Response> {
  const forms = await env.DB.prepare(
    'SELECT fingerprint, row_count, col_count, header_tokens, contributor_count FROM aggregated_forms ORDER BY contributor_count DESC LIMIT 100'
  ).all();

  const fields = await env.DB.prepare(
    'SELECT form_fingerprint, cell_addr, label_text, label_hash, canonical_field, cell_role, confidence, vote_count FROM aggregated_fields WHERE confidence >= 0.5 ORDER BY confidence DESC LIMIT 500'
  ).all();

  const policies = await env.DB.prepare(
    'SELECT family, biases_json FROM aggregated_policies ORDER BY contributor_count DESC LIMIT 50'
  ).all();

  // Merge all policy biases into one object
  const mergedPolicy: Record<string, any> = {};
  for (const row of (policies.results || [])) {
    const parsed = JSON.parse(row.biases_json as string);
    for (const [key, val] of Object.entries(parsed)) {
      if (!mergedPolicy[key]) mergedPolicy[key] = {};
      Object.assign(mergedPolicy[key], val);
    }
  }

  return json({
    version: 1,
    updatedAt: new Date().toISOString(),
    forms: (forms.results || []).map((r: any) => ({
      fingerprint: r.fingerprint,
      rowCount: r.row_count,
      colCount: r.col_count,
      headerTokens: JSON.parse(r.header_tokens || '[]'),
      contributorCount: r.contributor_count,
    })),
    fields: (fields.results || []).map((r: any) => ({
      formFingerprint: r.form_fingerprint,
      cellAddr: r.cell_addr,
      labelText: r.label_text,
      labelHash: r.label_hash,
      canonicalField: r.canonical_field,
      cellRole: r.cell_role,
      confidence: r.confidence,
      voteCount: r.vote_count,
    })),
    policy: mergedPolicy,
  }, 200, env);
}

async function handleStats(env: Env): Promise<Response> {
  const forms = await env.DB.prepare('SELECT COUNT(*) as cnt FROM aggregated_forms').first<{ cnt: number }>();
  const fields = await env.DB.prepare('SELECT COUNT(*) as cnt FROM aggregated_fields').first<{ cnt: number }>();
  const contributions = await env.DB.prepare('SELECT COUNT(*) as cnt FROM contributions').first<{ cnt: number }>();
  const contributors = await env.DB.prepare('SELECT COUNT(DISTINCT contributor_hash) as cnt FROM contributions').first<{ cnt: number }>();

  return json({
    forms: forms?.cnt || 0,
    fields: fields?.cnt || 0,
    contributions: contributions?.cnt || 0,
    contributors: contributors?.cnt || 0,
  }, 200, env);
}

function hashString(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash |= 0;
  }
  return 'h' + Math.abs(hash).toString(36);
}
