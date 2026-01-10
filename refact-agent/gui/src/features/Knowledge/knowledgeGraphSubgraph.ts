import type {
  KnowledgeGraphNode,
  KnowledgeGraphEdge,
} from "../../services/refact/types";

export type SubgraphParams = {
  seedId: string;
  depth: 1 | 2;
  nodes: KnowledgeGraphNode[];
  edges: KnowledgeGraphEdge[];
  includeNode: (node: KnowledgeGraphNode) => boolean;
};

export type SubgraphResult = {
  nodeIds: Set<string>;
  edgeIds: Set<string>;
};

export function buildSubgraph(params: SubgraphParams): SubgraphResult {
  const { seedId, depth, nodes, edges, includeNode } = params;

  const nodeMap = new Map<string, KnowledgeGraphNode>();
  nodes.forEach((node) => {
    nodeMap.set(node.id, node);
  });

  const seedNode = nodeMap.get(seedId);
  if (!seedNode) {
    return { nodeIds: new Set(), edgeIds: new Set() };
  }

  const adjacencyList = new Map<string, string[]>();
  const edgeMap = new Map<string, KnowledgeGraphEdge>();

  edges.forEach((edge) => {
    if (!nodeMap.has(edge.source) || !nodeMap.has(edge.target)) {
      return;
    }

    const edgeId = `${edge.source}|${edge.target}|${edge.edge_type}`;
    edgeMap.set(edgeId, edge);

    if (!adjacencyList.has(edge.source)) {
      adjacencyList.set(edge.source, []);
    }
    adjacencyList.get(edge.source)!.push(edge.target);

    if (!adjacencyList.has(edge.target)) {
      adjacencyList.set(edge.target, []);
    }
    adjacencyList.get(edge.target)!.push(edge.source);
  });

  const visitedNodes = new Set<string>();
  const resultNodeIds = new Set<string>();
  const resultEdgeIds = new Set<string>();

  const queue: Array<{ id: string; currentDepth: number }> = [
    { id: seedId, currentDepth: 0 },
  ];
  visitedNodes.add(seedId);
  resultNodeIds.add(seedId);

  while (queue.length > 0) {
    const current = queue.shift()!;

    if (current.currentDepth >= depth) {
      continue;
    }

    const neighbors = adjacencyList.get(current.id) || [];

    for (const neighborId of neighbors) {
      const neighborNode = nodeMap.get(neighborId);
      if (!neighborNode) continue;

      if (!includeNode(neighborNode)) continue;

      if (!visitedNodes.has(neighborId)) {
        visitedNodes.add(neighborId);
        resultNodeIds.add(neighborId);
        queue.push({ id: neighborId, currentDepth: current.currentDepth + 1 });
      }

      const forwardEdgeId = `${current.id}|${neighborId}|`;
      const backwardEdgeId = `${neighborId}|${current.id}|`;

      for (const [edgeId, edge] of edgeMap.entries()) {
        if (
          edgeId.startsWith(forwardEdgeId) ||
          edgeId.startsWith(backwardEdgeId)
        ) {
          if (
            resultNodeIds.has(edge.source) &&
            resultNodeIds.has(edge.target)
          ) {
            resultEdgeIds.add(edgeId);
          }
        }
      }
    }
  }

  return { nodeIds: resultNodeIds, edgeIds: resultEdgeIds };
}
